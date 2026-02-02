use crate::tree::Tree;

pub const DEFAULT_WIDTH: f64 = 3000.0;
pub const DEFAULT_HEIGHT: f64 = 2000.0;
pub const DEFAULT_GAP: f64 = 2.0;
pub const DEFAULT_PARENT_PAD: f64 = 2.0;
pub const DEFAULT_HEADER_HEIGHT: f64 = 16.0;

#[derive(Debug, Clone, Copy)]
pub struct LayoutConfig {
    pub width: f64,
    pub height: f64,
    pub gap: f64,
    pub parent_pad: f64,
    pub header_height: f64,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            gap: DEFAULT_GAP,
            parent_pad: DEFAULT_PARENT_PAD,
            header_height: DEFAULT_HEADER_HEIGHT,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Rect {
    pub name: String,
    pub display_ms: f64,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub is_self: bool,
    pub color: String,
}

#[derive(Clone, Copy)]
struct RectArea {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

pub fn layout_tree(tree: &Tree, config: &LayoutConfig) -> Vec<Rect> {
    let rect = RectArea {
        x: 0.0,
        y: 0.0,
        w: config.width,
        h: config.height,
    };
    let mut rects = Vec::new();
    layout_node(tree, tree.root, rect, &mut rects, config);
    rects
}

fn layout_node(tree: &Tree, index: usize, area: RectArea, rects: &mut Vec<Rect>, config: &LayoutConfig) {
    let node = &tree.arena[index];
    let total = tree.sum_children(index) as f64;
    if index != tree.root {
        let is_self = node.name == "self";
        let label = if is_self {
            parent_name(tree, index)
        } else {
            node.name.clone()
        };
        rects.push(Rect {
            name: label.clone(),
            display_ms: node.cumulative_us as f64 / 1000.0,
            x: area.x,
            y: area.y,
            w: area.w,
            h: area.h,
            is_self,
            color: color_for_name(&label, is_self),
        });
    }
    if node.children.is_empty() || total <= 0.0 {
        return;
    }
    let area = if index == tree.root {
        area
    } else {
        inset_area(area, config.parent_pad)
    };
    if area.w <= 0.0 || area.h <= 0.0 {
        return;
    }
    let area = if index == tree.root {
        area
    } else {
        reserve_header(area, config.header_height)
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
    let layout = squarify(children, area, total, config.gap);
    for (child_index, child_area) in layout {
        layout_node(tree, child_index, child_area, rects, config);
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

fn squarify(
    children: Vec<(usize, f64)>,
    area: RectArea,
    total: f64,
    gap: f64,
) -> Vec<(usize, RectArea)> {
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
            let (row_rects, rest) = layout_row(&row, current, gap);
            result.extend(row_rects);
            current = rest;
            row.clear();
        }
    }
    if !row.is_empty() {
        let (row_rects, _rest) = layout_row(&row, current, gap);
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

fn layout_row(row: &[(usize, f64)], area: RectArea, gap: f64) -> (Vec<(usize, RectArea)>, RectArea) {
    let row_area: f64 = row.iter().map(|(_, area)| area).sum();
    if row_area <= 0.0 {
        return (Vec::new(), area);
    }
    let mut rects = Vec::with_capacity(row.len());
    if area.w >= area.h {
        let gap_total = gap * (row.len().saturating_sub(1)) as f64;
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
            y += h + gap;
        }
        let rest = RectArea {
            x: area.x + row_w,
            y: area.y,
            w: area.w - row_w,
            h: area.h,
        };
        (rects, rest)
    } else {
        let gap_total = gap * (row.len().saturating_sub(1)) as f64;
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
            x += w + gap;
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
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::build_tree;

    #[test]
    fn layout_generates_rects() {
        let log = "\
import time: self [us] | cumulative | imported package\n\
import time:       10 |         10 | a\n\
import time:        5 |         15 | b\n\
import time:        3 |          3 |   b.c\n";
        let tree = build_tree(log).expect("tree");
        let rects = layout_tree(&tree, &LayoutConfig::default());
        assert!(!rects.is_empty());
        assert!(rects.iter().any(|rect| rect.name == "a"));
        assert!(rects.iter().any(|rect| rect.name == "b"));
    }
}
