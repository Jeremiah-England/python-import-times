use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

use crate::layout::{
    LayoutConfig, DEFAULT_GAP, DEFAULT_HEADER_HEIGHT, DEFAULT_HEIGHT, DEFAULT_PARENT_PAD,
    DEFAULT_WIDTH,
};
use crate::parser::{parse_import_time, ImportRecord};
use crate::render::{build_graph_html, build_graph_json};
use crate::util::{read_input, write_html_or_open, write_text_output};

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
        #[arg(long, default_value_t = true)]
        open: bool,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long, default_value_t = DEFAULT_WIDTH)]
        width: f64,
        #[arg(long, default_value_t = DEFAULT_HEIGHT)]
        height: f64,
        #[arg(long, default_value_t = DEFAULT_GAP)]
        gap: f64,
        #[arg(long, default_value_t = DEFAULT_PARENT_PAD)]
        parent_pad: f64,
        #[arg(long, default_value_t = DEFAULT_HEADER_HEIGHT)]
        header_height: f64,
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
        #[arg(long, default_value_t = DEFAULT_WIDTH)]
        width: f64,
        #[arg(long, default_value_t = DEFAULT_HEIGHT)]
        height: f64,
        #[arg(long, default_value_t = DEFAULT_GAP)]
        gap: f64,
        #[arg(long, default_value_t = DEFAULT_PARENT_PAD)]
        parent_pad: f64,
        #[arg(long, default_value_t = DEFAULT_HEADER_HEIGHT)]
        header_height: f64,
        #[arg(long, value_enum, default_value_t = OutputFormat::Html)]
        format: OutputFormat,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormat {
    Html,
    Json,
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

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run {
            python,
            open,
            output,
            width,
            height,
            gap,
            parent_pad,
            header_height,
            args,
        } => run_command(
            &python,
            args,
            output,
            open,
            LayoutConfig {
                width,
                height,
                gap,
                parent_pad,
                header_height,
            },
        ),
        Commands::Parse { input, output } => parse_command(&input, output),
        Commands::Graph {
            input,
            output,
            open,
            width,
            height,
            gap,
            parent_pad,
            header_height,
            format,
        } => graph_command(
            &input,
            output,
            open,
            format,
            LayoutConfig {
                width,
                height,
                gap,
                parent_pad,
                header_height,
            },
        ),
    }
}

fn run_command(
    python: &str,
    args: Vec<String>,
    output: Option<PathBuf>,
    open: bool,
    config: LayoutConfig,
) -> Result<()> {
    let (exe, exe_args) = resolve_executable_script(python, &args)?;
    let mut cmd = Command::new(exe);
    cmd.args(exe_args);
    cmd.env("PYTHONPROFILEIMPORTTIME", "1");
    let output_data = cmd.output().context("failed to run command")?;
    let text = String::from_utf8_lossy(&output_data.stderr).to_string();
    if !output_data.status.success() {
        eprintln!(
            "warning: command exited with status {}",
            output_data.status
        );
    }
    let html = build_graph_html(&text, &config)?;
    write_html_or_open(html, output, open)
}

fn resolve_executable_script(python: &str, args: &[String]) -> Result<(PathBuf, Vec<String>)> {
    if let Some(script_path) = find_python_script(args) {
        let mut script_args = Vec::with_capacity(args.len().saturating_sub(1));
        script_args.extend(args.iter().skip(1).cloned());
        return Ok((script_path, script_args));
    }
    Ok((PathBuf::from(python), args.to_vec()))
}

fn find_python_script(args: &[String]) -> Option<PathBuf> {
    let candidate = args.first()?;
    if candidate.starts_with('-') {
        return None;
    }
    let path = Path::new(candidate);
    if path.is_file() && is_python_shebang(path).unwrap_or(false) {
        return Some(path.to_path_buf());
    }
    let path = find_in_path(candidate)?;
    if is_python_shebang(&path).unwrap_or(false) {
        Some(path)
    } else {
        None
    }
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn is_python_shebang(path: &Path) -> Result<bool> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(false);
    }
    if !line.starts_with("#!") {
        return Ok(false);
    }
    let lower = line.to_ascii_lowercase();
    Ok(lower.contains("python"))
}

fn parse_command(input: &str, output: Option<PathBuf>) -> Result<()> {
    let text = read_input(input)?;
    let records = parse_import_time(&text)?;
    let json = ParseJson {
        records: records
            .into_iter()
            .map(record_to_json)
            .collect(),
    };
    write_text_output(serde_json::to_string_pretty(&json)?, output)
}

fn graph_command(
    input: &str,
    output: Option<PathBuf>,
    open: bool,
    format: OutputFormat,
    config: LayoutConfig,
) -> Result<()> {
    let text = read_input(input)?;
    match format {
        OutputFormat::Json => {
            let graph = build_graph_json(&text, &config)?;
            write_text_output(serde_json::to_string_pretty(&graph)?, output)
        }
        OutputFormat::Html => {
            let html = build_graph_html(&text, &config)?;
            write_html_or_open(html, output, open)
        }
    }
}

fn record_to_json(record: ImportRecord) -> ImportRecordJson {
    ImportRecordJson {
        name: record.name,
        self_us: record.self_us,
        cumulative_us: record.cumulative_us,
        depth: record.depth,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn make_temp_dir() -> PathBuf {
        let mut dir = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.push(format!("pyimporttime-test-{}-{}", std::process::id(), nanos));
        fs::create_dir(&dir).unwrap();
        dir
    }

    #[test]
    fn resolve_executable_script_prefers_shebang_script() {
        let dir = make_temp_dir();
        let script = dir.join("vs");
        fs::write(&script, "#!/usr/bin/env python\nprint('hi')\n").unwrap();

        let args = vec![
            script.to_string_lossy().to_string(),
            "arg1".to_string(),
        ];
        let (exe, exe_args) = resolve_executable_script("python", &args).unwrap();

        assert_eq!(exe, script);
        assert_eq!(exe_args, vec!["arg1".to_string()]);

        fs::remove_file(&exe).unwrap();
        fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn resolve_executable_script_falls_back_to_python() {
        let args = vec!["-c".to_string(), "print('hi')".to_string()];
        let (exe, exe_args) = resolve_executable_script("python3", &args).unwrap();

        assert_eq!(exe, PathBuf::from("python3"));
        assert_eq!(exe_args, args);
    }

    #[test]
    fn run_defaults_open_true() {
        let cli = Cli::parse_from(["pyimporttime", "run", "--", "-c", "print('hi')"]);
        match cli.command {
            Commands::Run { open, .. } => assert!(open),
            _ => panic!("expected run command"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn run_renders_on_nonzero_exit() {
        let dir = make_temp_dir();
        let script = dir.join("fake-python");
        let output = dir.join("out.html");
        let script_body = "\
#!/bin/sh
echo \"import time: self [us] | cumulative | imported package\" 1>&2
echo \"import time:       1 |          1 | a\" 1>&2
exit 2
";
        fs::write(&script, script_body).unwrap();
        let mut perms = fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).unwrap();

        let result = run_command(
            script.to_str().unwrap(),
            Vec::new(),
            Some(output.clone()),
            false,
            LayoutConfig {
                width: DEFAULT_WIDTH,
                height: DEFAULT_HEIGHT,
                gap: DEFAULT_GAP,
                parent_pad: DEFAULT_PARENT_PAD,
                header_height: DEFAULT_HEADER_HEIGHT,
            },
        );

        assert!(result.is_ok());
        assert!(output.is_file());

        fs::remove_file(&output).unwrap();
        fs::remove_file(&script).unwrap();
        fs::remove_dir(&dir).unwrap();
    }
}
