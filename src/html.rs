//! Self-contained interactive HTML viewer (no external dependencies).

use serde_json::{json, Value};

use crate::model::{GroupKind, Sheet, Topic};

const CSS: &str = include_str!("assets/viewer.css");
const JS: &str = include_str!("assets/viewer.js");

fn spec(sheets: &[Sheet], doc_title: &str) -> Value {
    fn conv(t: &Topic, uid: &mut u32) -> Value {
        *uid += 1;
        let id = *uid;
        let mut kids: Vec<Value> = t.children.iter().map(|c| conv(c, uid)).collect();
        for g in &t.groups {
            *uid += 1;
            let (kind, title) = match g.kind {
                GroupKind::Summary => ("summary", format!("Σ {}", g.title)),
                GroupKind::Boundary => ("group", "Group".to_string()),
            };
            kids.push(json!({
                "i": *uid, "k": kind, "t": title, "n": g.note,
                "c": t.covered_names(g), "ch": [],
            }));
        }
        json!({"i": id, "k": "topic", "t": t.title, "n": t.note, "m": t.markers, "ch": kids})
    }

    let mut uid = 0;
    let sheets: Vec<Value> = sheets
        .iter()
        .map(|s| json!({"t": s.title, "root": conv(&s.root, &mut uid)}))
        .collect();
    json!({"title": doc_title, "sheets": sheets})
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

pub fn html(sheets: &[Sheet], doc_title: &str) -> String {
    // keep the JSON inside <script type="application/json">
    let data = spec(sheets, doc_title).to_string().replace("</", "<\\/");
    let title = esc(doc_title);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{title}</title>
<style>{CSS}</style>
</head>
<body>
<header>
<h1>{title}</h1>
<nav id="tabs"></nav>
<div id="tools"><button id="zout" title="zoom out">−</button><button id="fit">fit</button><button id="zin" title="zoom in">＋</button><button id="exp">expand all</button><button id="col">collapse all</button></div>
<span class="leg">click a node to collapse/expand · hover for details · Σ = XMind summary · dashed = XMind boundary</span>
</header>
<div id="wrap"><svg id="sv" xmlns="http://www.w3.org/2000/svg"></svg></div>
<div id="info"></div>
<noscript><p style="padding:2em">This page needs JavaScript to draw the chart. Use the OPML or Markdown export for the plain outline.</p></noscript>
<script id="d" type="application/json">
{data}
</script>
<script>
{JS}
</script>
</body>
</html>
"#
    )
}
