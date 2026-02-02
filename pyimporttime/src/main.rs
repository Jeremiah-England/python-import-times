use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;

const DEFAULT_WIDTH: f64 = 3000.0;
const DEFAULT_HEIGHT: f64 = 2000.0;
const LAYOUT_GAP: f64 = 2.0;
const PARENT_PAD: f64 = 2.0;
const HEADER_HEIGHT: f64 = 16.0;

#[derive(Parser)]
#[command(name = "pyimporttime", version, about = "Python import time visualization")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(long, default_value = "python")]
        python: String,
        #[arg(long)]
        open: bool,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(last = true, required = true)]
        args: Vec<String>,
    },
    Parse {
        #[arg(value_name = "INPUT", default_value = "-")]
        input: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    Graph {
        #[arg(value_name = "INPUT", default_value = "-")]
        input: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        open: bool,
        #[arg(long, default_value = "html", value_parser = ["html", "json"])]
        format: String,
    },
}

#[derive(Debug, Clone)]
struct ImportRecord {
    name: String,
    self_us: u64,
    cumulative_us: u64,
    depth: usize,
}

#[derive(Debug)]
struct ArenaNode {
    name: String,
    cumulative_us: u64,
    parent: Option<usize>,
    children: Vec<usize>,
}

#[derive(Debug, Clone)]
struct Rect {
    name: String,
    display_ms: f64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    is_self: bool,
    color: String,
}

#[derive(Serialize)]
struct ParseJson {
    records: Vec<ImportRecordJson>,
}

#[derive(Serialize)]
struct ImportRecordJson {
    name: String,
    self_us: u64,
    cumulative_us: u64,
    depth: usize,
}

#[derive(Serialize)]
struct GraphJson {
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run {
            python,
            open,
            output,
            args,
        } => run_command(&python, args, output, open),
        Commands::Parse { input, output } => parse_command(&input, output),
        Commands::Graph {
            input,
            output,
            open,
            format,
        } => graph_command(&input, output, open, &format),
    }
}

fn run_command(python: &str, args: Vec<String>, output: Option<PathBuf>, open: bool) -> Result<()> {
    let mut cmd = Command::new(python);
    cmd.args(args);
    cmd.env("PYTHONPROFILEIMPORTTIME", "1");
    let output_data = cmd.output().context("failed to run python")?;
    if !output_data.status.success() {
        let stderr = String::from_utf8_lossy(&output_data.stderr);
        bail!("python exited with status {}: {}", output_data.status, stderr);
    }
    let text = String::from_utf8_lossy(&output_data.stderr).to_string();
    let html = build_graph_html(&text, DEFAULT_WIDTH, DEFAULT_HEIGHT)?;
    write_output_or_open(html, output, open)
}

fn parse_command(input: &str, output: Option<PathBuf>) -> Result<()> {
    let text = read_input(input)?;
    let records = parse_import_time(&text)?;
    let json = ParseJson {
        records: records
            .into_iter()
            .map(|record| ImportRecordJson {
                name: record.name,
                self_us: record.self_us,
                cumulative_us: record.cumulative_us,
                depth: record.depth,
            })
            .collect(),
    };
    write_text_output(serde_json::to_string_pretty(&json)?, output)
}

fn graph_command(input: &str, output: Option<PathBuf>, open: bool, format: &str) -> Result<()> {
    let text = read_input(input)?;
    if format == "json" {
        let graph = build_graph_json(&text, DEFAULT_WIDTH, DEFAULT_HEIGHT)?;
        return write_text_output(serde_json::to_string_pretty(&graph)?, output);
    }
    let html = build_graph_html(&text, DEFAULT_WIDTH, DEFAULT_HEIGHT)?;
    write_output_or_open(html, output, open)
}

fn read_input(input: &str) -> Result<String> {
    if input == "-" {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    } else {
        fs::read_to_string(input).with_context(|| format!("failed to read {}", input))
    }
}

fn write_text_output(text: String, output: Option<PathBuf>) -> Result<()> {
    if let Some(path) = output {
        fs::write(&path, text).with_context(|| format!("failed to write {}", path.display()))?;
    } else {
        io::stdout().write_all(text.as_bytes())?;
    }
    Ok(())
}

fn write_output_or_open(html: String, output: Option<PathBuf>, open: bool) -> Result<()> {
    if let Some(path) = output {
        fs::write(&path, html).with_context(|| format!("failed to write {}", path.display()))?;
        if open {
            open_in_browser(&path)?;
        }
        return Ok(());
    }
    if open {
        let temp = temp_html_path()?;
        fs::write(&temp, html).with_context(|| format!("failed to write {}", temp.display()))?;
        open_in_browser(&temp)?;
        println!("{}", temp.display());
    } else {
        io::stdout().write_all(html.as_bytes())?;
    }
    Ok(())
}

fn temp_html_path() -> Result<PathBuf> {
    let mut path = std::env::temp_dir();
    let file_name = format!("pyimporttime-{}.html", std::process::id());
    path.push(file_name);
    Ok(path)
}

fn open_in_browser(path: &Path) -> Result<()> {
    let status = Command::new("xdg-open")
        .arg(path)
        .status()
        .context("failed to run xdg-open")?;
    if !status.success() {
        bail!("xdg-open exited with status {}", status);
    }
    Ok(())
}

fn parse_import_time(text: &str) -> Result<Vec<ImportRecord>> {
    let mut records = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        if let Some(record) = parse_import_line(line) {
            records.push(record);
        } else if line.starts_with("import time:") {
            if line.contains("self [us]") {
                continue;
            }
            return Err(anyhow!("failed to parse import time on line {}", line_no + 1));
        }
    }
    if records.is_empty() {
        return Err(anyhow!("no import time records found"));
    }
    Ok(records)
}

fn parse_import_line(line: &str) -> Option<ImportRecord> {
    let prefix = "import time:";
    let stripped = line.strip_prefix(prefix)?;
    let mut parts = stripped.split('|').map(|part| part.trim_end());
    let self_part = parts.next()?.trim();
    let cumulative_part = parts.next()?.trim();
    let module_part = parts.next()?;
    if self_part.is_empty() || cumulative_part.is_empty() || module_part.is_empty() {
        return None;
    }
    let self_us = self_part.parse().ok()?;
    let cumulative_us = cumulative_part.parse().ok()?;
    let leading_spaces = module_part.chars().take_while(|c| *c == ' ').count();
    let name = module_part.trim().to_string();
    if name.is_empty() {
        return None;
    }
    let depth = (leading_spaces + 1) / 2;
    Some(ImportRecord {
        name,
        self_us,
        cumulative_us,
        depth,
    })
}

fn build_graph_json(text: &str, width: f64, height: f64) -> Result<GraphJson> {
    let tree = build_tree(text)?;
    let mut rects = Vec::new();
    layout_tree(&tree, width, height, &mut rects);
    let total_ms = tree.total_us() as f64 / 1000.0;
    Ok(GraphJson {
        meta: GraphMeta {
            title: "Python import time".to_string(),
            total_ms,
            width,
            height,
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

fn build_graph_html(text: &str, width: f64, height: f64) -> Result<String> {
    let tree = build_tree(text)?;
    let mut rects = Vec::new();
    layout_tree(&tree, width, height, &mut rects);
    let total_ms = tree.total_us() as f64 / 1000.0;
    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg id=\"import-graph\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" xmlns=\"http://www.w3.org/2000/svg\">"
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

struct Tree {
    arena: Vec<ArenaNode>,
    root: usize,
    totals: Vec<u64>,
}

impl Tree {
    fn total_us(&self) -> u64 {
        self.totals[self.root]
    }

    fn sum_children(&self, index: usize) -> u64 {
        self.totals[index]
    }
}

fn build_tree(text: &str) -> Result<Tree> {
    let records = parse_import_time(text)?;
    let mut arena = Vec::new();
        arena.push(ArenaNode {
            name: "Total".to_string(),
            cumulative_us: 0,
            parent: None,
            children: Vec::new(),
        });
    let root = 0;
    let mut stack: Vec<usize> = vec![root];
    for record in records {
        while stack.len() > record.depth {
            stack.pop();
        }
        let parent = *stack.last().unwrap_or(&root);
        let node_index = arena.len();
        arena.push(ArenaNode {
            name: record.name.clone(),
            cumulative_us: record.cumulative_us,
            parent: Some(parent),
            children: Vec::new(),
        });
        arena[parent].children.push(node_index);
        if record.self_us > 0 {
            let self_index = arena.len();
            arena.push(ArenaNode {
                name: "self".to_string(),
                cumulative_us: record.self_us,
                parent: Some(node_index),
                children: Vec::new(),
            });
            arena[node_index].children.push(self_index);
        }
        stack.push(node_index);
    }
    let mut tree = Tree {
        arena,
        root,
        totals: Vec::new(),
    };
    let mut totals = vec![0; tree.arena.len()];
    compute_totals(&tree.arena, root, &mut totals);
    tree.totals = totals;
    Ok(tree)
}

fn compute_totals(arena: &[ArenaNode], index: usize, totals: &mut [u64]) -> u64 {
    let node = &arena[index];
    if node.children.is_empty() {
        totals[index] = node.cumulative_us;
        return totals[index];
    }
    let mut sum = 0;
    for child in &node.children {
        sum += compute_totals(arena, *child, totals);
    }
    totals[index] = sum;
    sum
}

fn layout_tree(tree: &Tree, width: f64, height: f64, rects: &mut Vec<Rect>) {
    let rect = RectArea {
        x: 0.0,
        y: 0.0,
        w: width,
        h: height,
    };
    layout_node(tree, tree.root, rect, 0, rects);
}

#[derive(Clone, Copy)]
struct RectArea {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

fn layout_node(tree: &Tree, index: usize, area: RectArea, depth: usize, rects: &mut Vec<Rect>) {
    let node = &tree.arena[index];
    let total = tree.sum_children(index) as f64;
    if index != tree.root {
        let is_self = node.name == "self";
        let label = if is_self {
            parent_name(tree, index)
        } else {
            node.name.clone()
        };
        let color_base = label.clone();
        rects.push(Rect {
            name: label,
            display_ms: node.cumulative_us as f64 / 1000.0,
            x: area.x,
            y: area.y,
            w: area.w,
            h: area.h,
            is_self,
            color: color_for_name(&color_base, is_self),
        });
    }
    if node.children.is_empty() || total <= 0.0 {
        return;
    }
    let area = if index == tree.root {
        area
    } else {
        inset_area(area, PARENT_PAD)
    };
    if area.w <= 0.0 || area.h <= 0.0 {
        return;
    }
    let area = if index == tree.root {
        area
    } else {
        reserve_header(area, HEADER_HEIGHT)
    };
    if area.w <= 0.0 || area.h <= 0.0 {
        return;
    }
    let children: Vec<(usize, f64)> = node
        .children
        .iter()
        .filter_map(|child_index| {
            let child_total = tree.sum_children(*child_index) as f64;
            if child_total <= 0.0 {
                None
            } else {
                Some((*child_index, child_total))
            }
        })
        .collect();
    if children.is_empty() {
        return;
    }
    let layout = squarify(children, area, total);
    for (child_index, child_area) in layout {
        layout_node(tree, child_index, child_area, depth + 1, rects);
    }
}

fn inset_area(area: RectArea, pad: f64) -> RectArea {
    let w = (area.w - pad * 2.0).max(0.0);
    let h = (area.h - pad * 2.0).max(0.0);
    RectArea {
        x: area.x + pad,
        y: area.y + pad,
        w,
        h,
    }
}

fn reserve_header(area: RectArea, header_height: f64) -> RectArea {
    if area.h <= header_height + 2.0 {
        return area;
    }
    RectArea {
        x: area.x,
        y: area.y + header_height,
        w: area.w,
        h: area.h - header_height,
    }
}

fn squarify(children: Vec<(usize, f64)>, area: RectArea, total: f64) -> Vec<(usize, RectArea)> {
    if children.is_empty() || total <= 0.0 || area.w <= 0.0 || area.h <= 0.0 {
        return Vec::new();
    }
    let mut items: Vec<(usize, f64)> = children
        .into_iter()
        .map(|(index, weight)| (index, weight / total * area.w * area.h))
        .collect();
    items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut remaining = items.as_slice();
    let mut row: Vec<(usize, f64)> = Vec::new();
    let mut result: Vec<(usize, RectArea)> = Vec::new();
    let mut current = area;
    while !remaining.is_empty() {
        let item = remaining[0];
        if row.is_empty() {
            row.push(item);
            remaining = &remaining[1..];
            continue;
        }
        let side = current.w.min(current.h);
        let worst_current = worst_aspect(&row, side);
        let mut candidate = row.clone();
        candidate.push(item);
        let worst_candidate = worst_aspect(&candidate, side);
        if worst_candidate <= worst_current {
            row = candidate;
            remaining = &remaining[1..];
        } else {
            let (row_rects, rest) = layout_row(&row, current);
            result.extend(row_rects);
            current = rest;
            row.clear();
        }
    }
    if !row.is_empty() {
        let (row_rects, _rest) = layout_row(&row, current);
        result.extend(row_rects);
    }
    result
}

fn worst_aspect(row: &[(usize, f64)], side: f64) -> f64 {
    let mut max_area: f64 = 0.0;
    let mut min_area: f64 = f64::INFINITY;
    let mut sum: f64 = 0.0;
    for (_, area) in row {
        max_area = max_area.max(*area);
        min_area = min_area.min(*area);
        sum += *area;
    }
    if min_area <= 0.0 || side <= 0.0 {
        return f64::INFINITY;
    }
    let side2 = side * side;
    let sum2 = sum * sum;
    (side2 * max_area / sum2).max(sum2 / (side2 * min_area))
}

fn layout_row(row: &[(usize, f64)], area: RectArea) -> (Vec<(usize, RectArea)>, RectArea) {
    let row_area: f64 = row.iter().map(|(_, area)| area).sum();
    if row_area <= 0.0 {
        return (Vec::new(), area);
    }
    let mut rects = Vec::with_capacity(row.len());
    if area.w >= area.h {
        let gap_total = LAYOUT_GAP * (row.len().saturating_sub(1)) as f64;
        let available_h = (area.h - gap_total).max(0.0);
        if available_h <= 0.0 {
            return (Vec::new(), area);
        }
        let row_w = row_area / available_h;
        let mut y = area.y;
        for (index, item_area) in row {
            let h = item_area / row_w;
            rects.push((
                *index,
                RectArea {
                    x: area.x,
                    y,
                    w: row_w,
                    h,
                },
            ));
            y += h + LAYOUT_GAP;
        }
        let rest = RectArea {
            x: area.x + row_w,
            y: area.y,
            w: area.w - row_w,
            h: area.h,
        };
        (rects, rest)
    } else {
        let gap_total = LAYOUT_GAP * (row.len().saturating_sub(1)) as f64;
        let available_w = (area.w - gap_total).max(0.0);
        if available_w <= 0.0 {
            return (Vec::new(), area);
        }
        let row_h = row_area / available_w;
        let mut x = area.x;
        for (index, item_area) in row {
            let w = item_area / row_h;
            rects.push((
                *index,
                RectArea {
                    x,
                    y: area.y,
                    w,
                    h: row_h,
                },
            ));
            x += w + LAYOUT_GAP;
        }
        let rest = RectArea {
            x: area.x,
            y: area.y + row_h,
            w: area.w,
            h: area.h - row_h,
        };
        (rects, rest)
    }
}

fn parent_name(tree: &Tree, index: usize) -> String {
    let node = &tree.arena[index];
    let parent = node.parent.and_then(|p| tree.arena.get(p));
    parent.map_or_else(|| node.name.clone(), |p| p.name.clone())
}

fn color_for_name(name: &str, is_self: bool) -> String {
    let first = name.split('.').next().unwrap_or(name);
    let mut hash: i32 = 0;
    for ch in first.chars() {
        hash = hash.wrapping_mul(31).wrapping_add(ch as i32);
    }
    let hue = ((hash.wrapping_add(210)) % 360) as f64;
    let (sat, light) = if is_self { (0.35, 0.45) } else { (0.45, 0.5) };
    let (r, g, b) = hsl_to_rgb(hue / 360.0, sat, light);
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    if s == 0.0 {
        let v = (l * 255.0).round() as u8;
        return (v, v, v);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);
    (
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    )
}

fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> f64 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
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

    const SIMPLE_LOG: &str = "\
import time: self [us] | cumulative | imported package
import time:       10 |         10 | a
import time:        5 |         15 | b
import time:        3 |          3 |   b.c
";

    #[test]
    fn parse_import_line_basic() {
        let line = "import time:        8 |         12 |   pkg.mod";
        let record = parse_import_line(line).expect("record");
        assert_eq!(record.name, "pkg.mod");
        assert_eq!(record.self_us, 8);
        assert_eq!(record.cumulative_us, 12);
        assert_eq!(record.depth, 2);
    }

    #[test]
    fn parse_import_time_skips_header() {
        let records = parse_import_time(SIMPLE_LOG).expect("records");
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].name, "a");
        assert_eq!(records[1].name, "b");
        assert_eq!(records[2].name, "b.c");
    }

    #[test]
    fn build_tree_includes_self_nodes() {
        let tree = build_tree(SIMPLE_LOG).expect("tree");
        let names: Vec<&str> = tree.arena.iter().map(|node| node.name.as_str()).collect();
        assert!(names.contains(&"self"));
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
    }

    #[test]
    fn layout_generates_rects() {
        let tree = build_tree(SIMPLE_LOG).expect("tree");
        let mut rects = Vec::new();
        layout_tree(&tree, 300.0, 200.0, &mut rects);
        assert!(!rects.is_empty());
        assert!(rects.iter().any(|rect| rect.name == "a"));
        assert!(rects.iter().any(|rect| rect.name == "b"));
    }

    #[test]
    fn graph_html_contains_svg() {
        let html = build_graph_html(SIMPLE_LOG, 300.0, 200.0).expect("html");
        assert!(html.contains("<svg"));
        assert!(html.contains("import time"));
    }
}
