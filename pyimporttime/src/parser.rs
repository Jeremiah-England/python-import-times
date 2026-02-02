use anyhow::{anyhow, Result};

#[derive(Debug, Clone)]
pub struct ImportRecord {
    pub name: String,
    pub self_us: u64,
    pub cumulative_us: u64,
    pub depth: usize,
}

pub fn parse_import_time(text: &str) -> Result<Vec<ImportRecord>> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let log = "\
import time: self [us] | cumulative | imported package\n\
import time:       10 |         10 | a\n\
import time:        5 |         15 | b\n\
import time:        3 |          3 |   b.c\n";
        let records = parse_import_time(log).expect("records");
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].name, "a");
        assert_eq!(records[1].name, "b");
        assert_eq!(records[2].name, "b.c");
    }
}
