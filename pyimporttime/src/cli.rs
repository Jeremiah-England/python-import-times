use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Context, Result};
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
        #[arg(long)]
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
    let mut cmd = Command::new(python);
    cmd.args(args);
    cmd.env("PYTHONPROFILEIMPORTTIME", "1");
    let output_data = cmd.output().context("failed to run python")?;
    if !output_data.status.success() {
        let stderr = String::from_utf8_lossy(&output_data.stderr);
        bail!("python exited with status {}: {}", output_data.status, stderr);
    }
    let text = String::from_utf8_lossy(&output_data.stderr).to_string();
    let html = build_graph_html(&text, &config)?;
    write_html_or_open(html, output, open)
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
