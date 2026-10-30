//! Text emitters: OPML, FreeMind, Markdown, Mermaid, Graphviz DOT.
//!
//! Open outline/mind-map formats have no native "sibling-range" grouping, so
//! XMind summaries become an extra pseudo-child prefixed "Σ" on the parent
//! topic (with the covered siblings listed), and boundaries become "Group".

use std::collections::HashMap;

use crate::model::{Group, GroupKind, Sheet, Topic};

fn xml_escape(s: &str) -> String {
    let s: String = s
        .chars()
        .filter(|c| matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..))
        .collect();
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Escape a string for use inside a double-quoted XML attribute.
fn attr(s: &str) -> String {
    xml_escape(s).replace('\n', "&#10;")
}

fn group_label(g: &Group) -> String {
    match g.kind {
        GroupKind::Summary => format!("Σ {}", g.title),
        GroupKind::Boundary => "Group".to_string(),
    }
}

fn note_paragraphs(note: &str) -> String {
    note.split('\n')
        .filter(|l| !l.is_empty())
        .map(|l| format!("<p>{}</p>", xml_escape(l)))
        .collect()
}

// ---- OPML 2.0 ---------------------------------------------------------------
pub fn opml(sheets: &[Sheet], doc_title: &str, created: &str) -> String {
    let mut out = vec![
        r#"<?xml version="1.0" encoding="UTF-8"?>"#.to_string(),
        r#"<opml version="2.0">"#.to_string(),
        "  <head>".to_string(),
        format!("    <title>{}</title>", attr(doc_title)),
        format!("    <dateCreated>{created}</dateCreated>"),
        "  </head>".to_string(),
        "  <body>".to_string(),
    ];

    fn node(out: &mut Vec<String>, t: &Topic, indent: usize) {
        let pad = "  ".repeat(indent);
        let mut attrs = format!(r#" text="{}""#, attr(&t.title));
        if !t.note.is_empty() {
            attrs += &format!(r#" _note="{}""#, attr(&t.note));
        }
        if !t.markers.is_empty() {
            attrs += &format!(r#" _markers="{}""#, attr(&t.markers.join(" ")));
        }
        if t.children.is_empty() && t.groups.is_empty() {
            out.push(format!("{pad}<outline{attrs}/>"));
            return;
        }
        out.push(format!("{pad}<outline{attrs}>"));
        for c in &t.children {
            node(out, c, indent + 1);
        }
        for g in &t.groups {
            let mut gattrs = format!(
                r#" text="{}" _covers="{}""#,
                attr(&group_label(g)),
                attr(&t.covered_names(g))
            );
            if !g.note.is_empty() {
                gattrs += &format!(r#" _note="{}""#, attr(&g.note));
            }
            out.push(format!("{pad}  <outline{gattrs}/>"));
        }
        out.push(format!("{pad}</outline>"));
    }

    for s in sheets {
        out.push(format!(r#"  <outline text="{}">"#, attr(&s.title)));
        node(&mut out, &s.root, 2);
        out.push("  </outline>".to_string());
    }
    out.push("  </body>".to_string());
    out.push("</opml>".to_string());
    out.join("\n") + "\n"
}

// ---- FreeMind .mm -------------------------------------------------------------
pub fn freemind(sheet: &Sheet) -> String {
    let mut out = vec![
        r#"<?xml version="1.0" encoding="UTF-8"?>"#.to_string(),
        r#"<map version="1.0.1">"#.to_string(),
    ];

    fn note_node(pad: &str, note: &str) -> String {
        format!(
            r#"{pad}<richcontent TYPE="NOTE"><html><head></head><body>{}</body></html></richcontent>"#,
            note_paragraphs(note)
        )
    }

    fn node(out: &mut Vec<String>, t: &Topic, indent: usize) {
        let pad = "  ".repeat(indent);
        out.push(format!(r#"{pad}<node TEXT="{}">"#, attr(&t.title)));
        if !t.note.is_empty() {
            out.push(note_node(&format!("{pad}  "), &t.note));
        }
        if !t.markers.is_empty() {
            out.push(format!(
                r#"{pad}  <attribute NAME="markers" VALUE="{}"/>"#,
                attr(&t.markers.join(" "))
            ));
        }
        for c in &t.children {
            node(out, c, indent + 1);
        }
        for g in &t.groups {
            let kind = match g.kind {
                GroupKind::Summary => "summary",
                GroupKind::Boundary => "boundary",
            };
            out.push(format!(r#"{pad}  <node TEXT="{}">"#, attr(&group_label(g))));
            out.push(format!(
                r#"{pad}    <attribute NAME="{kind}-covers" VALUE="{}"/>"#,
                attr(&t.covered_names(g))
            ));
            if !g.note.is_empty() {
                out.push(note_node(&format!("{pad}    "), &g.note));
            }
            out.push(format!("{pad}  </node>"));
        }
        out.push(format!("{pad}</node>"));
    }

    node(&mut out, &sheet.root, 1);
    out.push("</map>".to_string());
    out.join("\n") + "\n"
}

// ---- Markdown -----------------------------------------------------------------
pub fn markdown(sheets: &[Sheet], doc_title: &str) -> String {
    let mut out = vec![format!("# {doc_title}"), String::new()];

    fn quote(out: &mut Vec<String>, pad: &str, note: &str) {
        for ln in note.split('\n') {
            out.push(format!("{pad}  > {ln}"));
        }
    }

    fn node(out: &mut Vec<String>, t: &Topic, indent: usize) {
        let pad = "  ".repeat(indent);
        let mut text = t.title.replace('\n', " / ");
        if !t.markers.is_empty() {
            text += &format!(" · marker: {}", t.markers.join(", "));
        }
        if t.note.is_empty() {
            out.push(format!("{pad}- {text}"));
        } else {
            out.push(format!("{pad}- {text} *📝*"));
            out.push(String::new());
            quote(out, &pad, &t.note);
        }
        for c in &t.children {
            node(out, c, indent + 1);
        }
        for g in &t.groups {
            let label = match g.kind {
                GroupKind::Summary => format!("Σ **{}**", g.title),
                GroupKind::Boundary => "**Group**".to_string(),
            };
            out.push(format!("{pad}- {label} — covers: *{}*", t.covered_names(g)));
            if !g.note.is_empty() {
                out.push(String::new());
                quote(out, &pad, &g.note);
            }
        }
    }

    for s in sheets {
        out.push(format!("## {}", s.title));
        out.push(String::new());
        node(&mut out, &s.root, 0);
        out.push(String::new());
    }
    out.join("\n").trim_end().to_string() + "\n"
}

// ---- shared id bookkeeping for the graph emitters ---------------------------------
/// Pre-order topic ids (`t1`, `t2`, ...) keyed by node address, drawing from a
/// counter that is also used for note/group/summary pseudo-nodes.
struct Ids {
    counter: usize,
    topics: HashMap<*const Topic, String>,
}

impl Ids {
    fn new() -> Self {
        Ids { counter: 0, topics: HashMap::new() }
    }

    fn fresh(&mut self, prefix: &str) -> String {
        self.counter += 1;
        format!("{prefix}{}", self.counter)
    }

    fn reserve(&mut self, t: &Topic) {
        let id = self.fresh("t");
        self.topics.insert(t as *const Topic, id);
        for c in &t.children {
            self.reserve(c);
        }
    }

    fn of(&self, t: &Topic) -> &str {
        &self.topics[&(t as *const Topic)]
    }
}

/// Boundary start/end markers keyed by child index.
fn boundary_spans(
    t: &Topic,
    mut next_id: impl FnMut() -> String,
) -> (HashMap<usize, Vec<String>>, HashMap<usize, Vec<String>>) {
    let (mut starts, mut ends) = (HashMap::new(), HashMap::new());
    if t.children.is_empty() {
        return (starts, ends);
    }
    for g in t.groups.iter().filter(|g| g.kind == GroupKind::Boundary) {
        let gid = next_id();
        let end = g.end.min(t.children.len() as i64 - 1);
        starts.entry(g.start.max(0) as usize).or_insert_with(Vec::new).push(gid.clone());
        ends.entry(end.max(0) as usize).or_insert_with(Vec::new).push(gid);
    }
    (starts, ends)
}

// ---- Mermaid (.mmd) -------------------------------------------------------------
fn mermaid_label(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "#lt;")
        .replace('>', "#gt;")
        .replace('"', "#quot;")
        .replace('\n', "<br/>")
}

pub fn mermaid(sheet: &Sheet) -> String {
    let mut out = vec![
        format!("%% XMind sheet: {}", sheet.title),
        "graph TD".to_string(),
        "  classDef summary fill:#FFF3B0,stroke:#B7860B,stroke-dasharray:4 3;".to_string(),
        "  classDef note fill:#FFFFFE,stroke:#999999,stroke-dasharray:2 2;".to_string(),
        "  classDef boundary fill:#EFEFEF,stroke:#888888;".to_string(),
    ];
    let mut ids = Ids::new();
    ids.reserve(&sheet.root);

    fn draw(out: &mut Vec<String>, ids: &mut Ids, t: &Topic, parent: Option<&str>) {
        let me = ids.of(t).to_string();
        out.push(format!(r#"  {me}["{}"]"#, mermaid_label(&t.label())));
        if let Some(p) = parent {
            out.push(format!("  {p} --> {me}"));
        }
        if !t.note.is_empty() {
            let nid = ids.fresh("n");
            out.push(format!(r#"  {nid}("📝 {}")"#, mermaid_label(&t.note)));
            out.push(format!("  {me} -.-> {nid}"));
            out.push(format!("  {nid}:::note"));
        }

        let (starts, ends) = boundary_spans(t, || ids.fresh("b"));
        for (i, c) in t.children.iter().enumerate() {
            for gid in starts.get(&i).into_iter().flatten() {
                out.push(format!(r#"  subgraph {gid}["Group"]"#));
                out.push("    direction TB".to_string());
            }
            draw(out, ids, c, Some(&me));
            for _ in ends.get(&i).into_iter().flatten() {
                out.push("  end".to_string());
            }
        }

        for g in t.groups.iter().filter(|g| g.kind == GroupKind::Summary) {
            let gid = ids.fresh("s");
            out.push(format!(r#"  {gid}("Σ {}")"#, mermaid_label(&g.title)));
            out.push(format!("  {gid}:::summary"));
            out.push(format!("  {me} --- {gid}"));
            for i in t.covered(g) {
                out.push(format!("  {gid} -.-> {}", ids.of(&t.children[i])));
            }
            if !g.note.is_empty() {
                let nid = ids.fresh("n");
                out.push(format!(r#"  {nid}("📝 {}")"#, mermaid_label(&g.note)));
                out.push(format!("  {gid} -.-> {nid}"));
                out.push(format!("  {nid}:::note"));
            }
        }
    }

    draw(&mut out, &mut ids, &sheet.root, None);
    out.join("\n") + "\n"
}

// ---- Graphviz DOT ----------------------------------------------------------------
fn dot_label(s: &str) -> String {
    const WRAP: usize = 52;
    let s = s.replace('\\', "\\\\").replace('"', "\\\"");
    let mut parts: Vec<String> = Vec::new();
    for para in s.split('\n') {
        let mut cur = String::new();
        let mut cur_len = 0;
        for w in para.split(' ') {
            let w_len = w.chars().count();
            if !cur.is_empty() && cur_len + w_len + 1 > WRAP {
                parts.push(std::mem::take(&mut cur));
                cur = w.to_string();
                cur_len = w_len;
            } else if cur.is_empty() {
                cur = w.to_string();
                cur_len = w_len;
            } else {
                cur = format!("{cur} {w}");
                cur_len += w_len + 1;
            }
        }
        parts.push(cur);
    }
    let joined = parts.join("\\n");
    if joined.is_empty() { " ".to_string() } else { joined }
}

const DOT_NOTE_STYLE: &str = r##"shape=note, style="filled,dashed", fillcolor="#FFFFFE", color="#999999""##;

/// `rankdir` is "TB" for the combined text file, "LR" for per-sheet images.
pub fn dot(sheets: &[Sheet], rankdir: &str) -> String {
    let mut out = vec![
        r#"digraph "Org Chart" {"#.to_string(),
        format!("  graph [rankdir={rankdir}, splines=ortho, nodesep=0.25, ranksep=0.6, compound=true];"),
        r##"  node  [shape=box, style="rounded,filled", fillcolor="#DDEBF7","##.to_string(),
        r##"         color="#2E5B88", fontname="Helvetica", fontsize=11];"##.to_string(),
        r##"  edge  [arrowhead=none, color="#7F7F7F"];"##.to_string(),
    ];

    struct Ctx {
        ids: Ids,
        clusters: usize,
    }

    fn draw(out: &mut Vec<String>, cx: &mut Ctx, t: &Topic, parent: Option<&str>) {
        let me = cx.ids.of(t).to_string();
        out.push(format!(r#"  {me} [label="{}"];"#, dot_label(&t.label())));
        if let Some(p) = parent {
            out.push(format!("  {p} -> {me};"));
        }
        if !t.note.is_empty() {
            let nid = cx.ids.fresh("n");
            out.push(format!(r#"  {nid} [label="📝 {}", {DOT_NOTE_STYLE}];"#, dot_label(&t.note)));
            out.push(format!("  {me} -> {nid} [style=dotted, arrowhead=normal];"));
        }

        let (starts, ends) = boundary_spans(t, || {
            cx.clusters += 1;
            format!("cluster_{}", cx.clusters)
        });
        for (i, c) in t.children.iter().enumerate() {
            for gid in starts.get(&i).into_iter().flatten() {
                out.push(format!("  subgraph {gid} {{"));
                out.push(r##"    label="Group"; color="#AAAAAA"; style="rounded,dashed";"##.to_string());
            }
            draw(out, cx, c, Some(&me));
            for _ in ends.get(&i).into_iter().flatten() {
                out.push("  }".to_string());
            }
        }

        for g in t.groups.iter().filter(|g| g.kind == GroupKind::Summary) {
            let gid = cx.ids.fresh("s");
            let title = if g.title.is_empty() { "Summary" } else { &g.title };
            out.push(format!(
                r##"  {gid} [label="{}", shape=pentagon, fillcolor="#FFF2CC", color="#BF8F00"];"##,
                dot_label(&format!("Σ {title}"))
            ));
            out.push(format!("  {me} -> {gid};"));
            for i in t.covered(g) {
                out.push(format!(
                    r##"  {gid} -> {} [style=dotted, arrowhead=normal, color="#BF8F00"];"##,
                    cx.ids.of(&t.children[i])
                ));
            }
            if !g.note.is_empty() {
                let nid = cx.ids.fresh("n");
                out.push(format!(r#"  {nid} [label="📝 {}", {DOT_NOTE_STYLE}];"#, dot_label(&g.note)));
                out.push(format!("  {gid} -> {nid} [style=dotted, arrowhead=normal];"));
            }
        }
    }

    let mut cx = Ctx { ids: Ids::new(), clusters: 0 };
    for s in sheets {
        cx.ids.reserve(&s.root);
        draw(&mut out, &mut cx, &s.root, None);
    }
    out.push("}".to_string());
    out.join("\n") + "\n"
}
