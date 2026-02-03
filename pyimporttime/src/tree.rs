use anyhow::Result;

use crate::parser::{parse_import_time, ImportRecord};

#[derive(Debug)]
pub struct ArenaNode {
    pub(crate) name: String,
    pub(crate) cumulative_us: u64,
    pub(crate) parent: Option<usize>,
    pub(crate) children: Vec<usize>,
}

#[derive(Debug)]
pub struct Tree {
    pub(crate) arena: Vec<ArenaNode>,
    pub(crate) root: usize,
    pub(crate) totals: Vec<u64>,
}

impl Tree {
    pub fn total_us(&self) -> u64 {
        self.totals[self.root]
    }

    pub(crate) fn sum_children(&self, index: usize) -> u64 {
        self.totals[index]
    }
}

pub fn build_tree(text: &str) -> Result<Tree> {
    let mut records = parse_import_time(text)?;
    // Import time logs are emitted after child imports complete, so reverse to build a pre-order tree.
    records.reverse();
    build_tree_from_records(&records)
}

fn build_tree_from_records(records: &[ImportRecord]) -> Result<Tree> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_tree_includes_self_nodes() {
        let log = "\
import time: self [us] | cumulative | imported package\n\
import time:       10 |         10 | a\n\
import time:        5 |         15 | b\n\
import time:        3 |          3 |   b.c\n";
        let tree = build_tree(log).expect("tree");
        let names: Vec<&str> = tree.arena.iter().map(|node| node.name.as_str()).collect();
        assert!(names.contains(&"self"));
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
    }

    #[test]
    fn build_tree_handles_postorder_logs() {
        let log = "\
import time: self [us] | cumulative | imported package\n\
import time:        1 |          1 |   child\n\
import time:        2 |          3 | parent\n";
        let tree = build_tree(log).expect("tree");
        let parent_index = tree
            .arena
            .iter()
            .position(|node| node.name == "parent")
            .expect("parent");
        let child_index = tree
            .arena
            .iter()
            .position(|node| node.name == "child")
            .expect("child");
        assert!(tree.arena[parent_index].children.contains(&child_index));
    }
}
