//! Reads `.xmind` workbooks (zip containers) in both flavours:
//! Xmind Zen / 2020+ (`content.json`) and XMind 8 (`content.xml`).

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use roxmltree::Node;
use serde_json::Value;

use crate::model::{Group, GroupKind, Sheet, Topic};

const X8_NS: &str = "urn:xmind:xmap:xmlns:content:2.0";

pub fn load(path: &Path) -> Result<Vec<Sheet>> {
    let file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file)
        .with_context(|| format!("{} is not a valid .xmind (zip) file", path.display()))?;

    if let Ok(mut entry) = zip.by_name("content.json") {
        let mut raw = String::new();
        entry.read_to_string(&mut raw)?;
        return load_json(&raw);
    }
    if let Ok(mut entry) = zip.by_name("content.xml") {
        let mut raw = String::new();
        entry.read_to_string(&mut raw)?;
        return load_xml(&raw);
    }
    bail!("{}: neither content.json nor content.xml found", path.display())
}

/// Parses an XMind range spec such as `(0,2)`.
fn parse_range(spec: &str) -> Option<(i64, i64)> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\(\s*(\d+)\s*,\s*(\d+)\s*\)").unwrap());
    let c = re.captures(spec)?;
    Some((c[1].parse().ok()?, c[2].parse().ok()?))
}

/// Crude-but-safe HTML -> text. Handles namespaced tags (xhtml `html:p`)
/// as found in XMind 8 notes, and preserves paragraph spacing.
fn html_to_text(raw: &str) -> String {
    static RE: OnceLock<[Regex; 3]> = OnceLock::new();
    let [br, block_end, tag] = RE.get_or_init(|| {
        [
            Regex::new(r"(?i)<(?:[\w\-]+:)?br\s*/?>").unwrap(),
            Regex::new(r"(?i)</(?:[\w\-]+:)?(?:p|div|li|h\d)\b[^>]*>").unwrap(),
            Regex::new(r"<[^>]+>").unwrap(),
        ]
    });
    let text = br.replace_all(raw, "\n");
    let text = block_end.replace_all(&text, "\n");
    let text = tag.replace_all(&text, "");
    let text = html_escape::decode_html_entities(&text);

    let mut out: Vec<&str> = Vec::new();
    let mut blanks = 0;
    for line in text.split('\n').map(str::trim) {
        if line.is_empty() {
            blanks += 1;
            if blanks == 1 {
                out.push("");
            }
            continue;
        }
        blanks = 0;
        out.push(line);
    }
    while out.last() == Some(&"") {
        out.pop();
    }
    while out.first() == Some(&"") {
        out.remove(0);
    }
    out.join("\n").trim().to_string()
}

// ---- Xmind Zen / 2020+ (JSON) ------------------------------------------------

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn json_note(topic: &Value) -> String {
    let notes = &topic["notes"];
    if let Some(plain) = notes["plain"]["content"].as_str() {
        if !plain.trim().is_empty() {
            return plain.trim().to_string();
        }
    }
    notes["realHTML"]["content"]
        .as_str()
        .map(html_to_text)
        .unwrap_or_default()
}

fn json_markers(topic: &Value) -> Vec<String> {
    match &topic["markers"] {
        // Zen: [{"markerId": "priority-1"}, ...]
        Value::Array(list) => list
            .iter()
            .filter_map(|m| m["markerId"].as_str().map(str::to_string))
            .collect(),
        Value::Object(map) => map
            .iter()
            .filter(|(_, v)| !v.is_null() && **v != Value::Bool(false))
            .map(|(k, _)| k.clone())
            .collect(),
        _ => Vec::new(),
    }
}

fn json_topic(tj: &Value) -> Topic {
    let title = str_of(tj, "title").trim();
    let kids = &tj["children"];
    let list = |key: &str| kids[key].as_array().cloned().unwrap_or_default();

    let mut topic = Topic {
        title: if title.is_empty() { "(untitled)" } else { title }.to_string(),
        note: json_note(tj),
        markers: json_markers(tj),
        children: list("attached").iter().map(json_topic).collect(),
        groups: Vec::new(),
    };

    let summary_topics: HashMap<String, Value> = list("summary")
        .into_iter()
        .filter_map(|s| Some((s["id"].as_str()?.to_string(), s)))
        .collect();

    for (key, kind) in [
        ("summaries", GroupKind::Summary),
        ("boundaries", GroupKind::Boundary),
    ] {
        for gj in tj[key].as_array().into_iter().flatten() {
            let Some((start, end)) = parse_range(str_of(gj, "range")) else {
                continue;
            };
            let mut title = str_of(gj, "title").to_string();
            let mut note = String::new();
            if kind == GroupKind::Summary {
                let stj = summary_topics
                    .get(str_of(gj, "topicId"))
                    .cloned()
                    .unwrap_or(Value::Null);
                title = match str_of(&stj, "title") {
                    "" => "(untitled summary)".to_string(),
                    t => t.to_string(),
                };
                note = json_note(&stj);
            }
            topic.groups.push(Group { kind, start, end, title, note });
        }
    }
    sort_groups(&mut topic.groups);
    topic
}

fn sort_groups(groups: &mut [Group]) {
    // by start index, summaries before boundaries
    groups.sort_by_key(|g| (g.start, g.kind != GroupKind::Summary));
}

fn load_json(raw: &str) -> Result<Vec<Sheet>> {
    let content: Value = serde_json::from_str(raw).context("invalid content.json")?;
    let sheets = content
        .as_array()
        .ok_or_else(|| anyhow!("content.json: expected a list of sheets"))?;
    sheets
        .iter()
        .enumerate()
        .map(|(i, sheet)| {
            let title = match str_of(sheet, "title") {
                "" => format!("Sheet {}", i + 1),
                t => t.to_string(),
            };
            if sheet["rootTopic"].is_null() {
                bail!("sheet {} has no rootTopic", i + 1);
            }
            Ok(Sheet { title, root: json_topic(&sheet["rootTopic"]) })
        })
        .collect()
}

// ---- XMind 8 (XML) -----------------------------------------------------------

fn child<'a, 'i>(el: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    el.children()
        .find(|n| n.is_element() && n.tag_name().name() == name && n.tag_name().namespace() == Some(X8_NS))
}

fn children<'a, 'i>(el: Node<'a, 'i>, name: &'a str) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
    el.children()
        .filter(move |n| n.is_element() && n.tag_name().name() == name && n.tag_name().namespace() == Some(X8_NS))
}

fn itertext(el: Node) -> String {
    el.descendants().filter(|n| n.is_text()).filter_map(|n| n.text()).collect()
}

fn xml_title(el: Node) -> String {
    child(el, "title").map(|t| itertext(t).trim().to_string()).unwrap_or_default()
}

fn xml_notes(el: Node, src: &str) -> String {
    let Some(notes) = child(el, "notes") else {
        return String::new();
    };
    if let Some(html) = child(notes, "html") {
        return html_to_text(&src[html.range()]);
    }
    if let Some(plain) = child(notes, "plain") {
        return itertext(plain).trim().to_string();
    }
    String::new()
}

fn xml_topic(el: Node, src: &str) -> Topic {
    let title = xml_title(el);
    let markers = children(el, "marker-refs")
        .flat_map(|b| children(b, "marker-ref"))
        .filter_map(|m| m.attribute("marker-id"))
        .map(str::to_string)
        .collect();

    // later <topics> blocks of the same type replace earlier ones
    let mut by_kind: HashMap<&str, Vec<Topic>> = HashMap::new();
    let mut summary_els: HashMap<&str, Node> = HashMap::new();
    if let Some(kids) = child(el, "children") {
        for block in children(kids, "topics") {
            let kind = block.attribute("type").unwrap_or("attached");
            let topics: Vec<_> = children(block, "topic").collect();
            if kind == "summary" {
                summary_els = topics.iter().filter_map(|t| Some((t.attribute("id")?, *t))).collect();
            }
            by_kind.insert(kind, topics.iter().map(|t| xml_topic(*t, src)).collect());
        }
    }
    let mut kids = by_kind.remove("attached").unwrap_or_default();
    kids.extend(by_kind.remove("detached").unwrap_or_default());

    let mut groups = Vec::new();
    for s in children(el, "summaries").flat_map(|b| children(b, "summary")) {
        let Some((start, end)) = s.attribute("range").and_then(parse_range) else {
            continue;
        };
        let st = s.attribute("topic-id").and_then(|id| summary_els.get(id));
        let (title, note) = match st {
            Some(st) => (xml_title(*st), xml_notes(*st, src)),
            None => match xml_title(s) {
                t if t.is_empty() => ("(untitled summary)".to_string(), String::new()),
                t => (t, String::new()),
            },
        };
        groups.push(Group { kind: GroupKind::Summary, start, end, title, note });
    }
    for b in children(el, "boundaries").flat_map(|b| children(b, "boundary")) {
        if let Some((start, end)) = b.attribute("range").and_then(parse_range) {
            groups.push(Group { kind: GroupKind::Boundary, start, end, title: xml_title(b), note: String::new() });
        }
    }
    sort_groups(&mut groups);

    Topic {
        title: if title.is_empty() { "(untitled)".to_string() } else { title },
        note: xml_notes(el, src),
        children: kids,
        groups,
        markers,
    }
}

fn load_xml(raw: &str) -> Result<Vec<Sheet>> {
    let doc = roxmltree::Document::parse_with_options(
        raw,
        roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() },
    )
    .context("invalid content.xml")?;
    let mut sheets = Vec::new();
    for (i, sheet) in children(doc.root_element(), "sheet").enumerate() {
        let topic_el = child(sheet, "topic").ok_or_else(|| anyhow!("sheet {} has no root topic", i + 1))?;
        let root = xml_topic(topic_el, raw);
        let title = [
            sheet.attribute("title").unwrap_or("").to_string(),
            xml_title(sheet),
            root.title.clone(),
        ]
        .into_iter()
        .find(|t| !t.is_empty())
        .unwrap_or_else(|| format!("Sheet {}", i + 1));
        sheets.push(Sheet { title, root });
    }
    Ok(sheets)
}
