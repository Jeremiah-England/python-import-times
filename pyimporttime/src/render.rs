use anyhow::Result;
use serde::Serialize;

use crate::layout::{layout_tree, LayoutConfig, Rect};
use crate::tree::build_tree;

#[derive(Serialize)]
pub struct GraphJson {
    meta: GraphMeta,
    rects: Vec<GraphRect>,
}

#[derive(Serialize)]
struct GraphMeta {
    title: String,
    total_ms: f64,
    width: f64,
    height: f64,
}

#[derive(Serialize)]
struct GraphRect {
    label: String,
    ms: f64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    color: String,
}

pub fn build_graph_json(text: &str, config: &LayoutConfig) -> Result<GraphJson> {
    let tree = build_tree(text)?;
    let rects = layout_tree(&tree, config);
    let total_ms = tree.total_us() as f64 / 1000.0;
    Ok(GraphJson {
        meta: GraphMeta {
            title: "Python import time".to_string(),
            total_ms,
            width: config.width,
            height: config.height,
        },
        rects: rects
            .into_iter()
            .map(|rect| GraphRect {
                label: rect.name,
                ms: rect.display_ms,
                x: rect.x,
                y: rect.y,
                w: rect.w,
                h: rect.h,
                color: rect.color,
            })
            .collect(),
    })
}

pub fn build_graph_html(text: &str, config: &LayoutConfig) -> Result<String> {
    let tree = build_tree(text)?;
    let rects = layout_tree(&tree, config);
    let total_ms = tree.total_us() as f64 / 1000.0;
    let svg = render_svg(&rects, config);
    let html = format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><title>Python import time</title><style>\
        body{{margin:0;padding:0;background:#333;color:#eee;font-family:sans-serif;}}\
        #toolbar{{height:36px;line-height:36px;background:#444;padding:0 12px;font-size:14px;}}\
        #graph-wrap{{overflow:auto;}}\
        </style></head><body>\
        <div id=\"toolbar\">Python import time - total {:.3} ms</div>\
        <div id=\"graph-wrap\">{}</div></body></html>",
        total_ms, svg
    );
    Ok(html)
}

fn render_svg(rects: &[Rect], config: &LayoutConfig) -> String {
    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg id=\"import-graph\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" xmlns=\"http://www.w3.org/2000/svg\">",
        width = config.width,
        height = config.height
    ));
    svg.push_str("<rect x=\"0\" y=\"0\" width=\"100%\" height=\"100%\" fill=\"#333\"/>");
    for rect in rects {
        let name = escape_xml(&rect.name);
        let title_label = if rect.is_self {
            format!("{} (self)", rect.name)
        } else {
            rect.name.clone()
        };
        let title = escape_xml(&format!("{}: {:.3} ms", title_label, rect.display_ms));
        let stroke = if rect.is_self { "none" } else { "#fff" };
        svg.push_str(&format!(
            "<g transform=\"translate({:.2},{:.2})\">",
            rect.x, rect.y
        ));
        svg.push_str(&format!(
            "<rect width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\" stroke=\"{}\"/>",
            rect.w, rect.h, rect.color, stroke
        ));
        svg.push_str(&format!("<title>{}</title>", title));
        if !rect.is_self && rect.w > 40.0 && rect.h > 16.0 {
            svg.push_str(&format!(
                "<text x=\"4\" y=\"14\" fill=\"#fff\" font-size=\"10\" font-family=\"sans-serif\">{}: {:.3} ms</text>",
                name, rect.display_ms
            ));
        }
        svg.push_str("</g>");
    }
    svg.push_str("</svg>");
    svg
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::LayoutConfig;

    #[test]
    fn graph_html_contains_svg() {
        let log = "\
import time: self [us] | cumulative | imported package\n\
import time:       10 |         10 | a\n";
        let html = build_graph_html(log, &LayoutConfig::default()).expect("html");
        assert!(html.contains("<svg"));
        assert!(html.contains("import time"));
    }
}
