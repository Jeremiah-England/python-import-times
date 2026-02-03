use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

pub fn read_input(input: &str) -> Result<String> {
    if input == "-" {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    } else {
        fs::read_to_string(input).with_context(|| format!("failed to read {}", input))
    }
}

pub fn write_text_output(text: String, output: Option<PathBuf>) -> Result<()> {
    if let Some(path) = output {
        fs::write(&path, text).with_context(|| format!("failed to write {}", path.display()))?;
    } else {
        io::stdout().write_all(text.as_bytes())?;
    }
    Ok(())
}

pub fn write_html_or_open(html: String, output: Option<PathBuf>, open: bool) -> Result<()> {
    let target = html_output_target(output)?;
    write_html_to_target(&html, &target)?;
    let path = target.path();
    if open {
        if let Err(err) = open_in_browser(path) {
            eprintln!("warning: failed to open browser: {err}");
        }
    }
    println!("{}", path.display());
    Ok(())
}

fn temp_html_path() -> Result<PathBuf> {
    let mut path = std::env::temp_dir();
    let file_name = format!("pyimporttime-{}.html", std::process::id());
    path.push(file_name);
    Ok(path)
}

enum HtmlOutputTarget {
    Path(PathBuf),
    Temp(PathBuf),
}

impl HtmlOutputTarget {
    fn path(&self) -> &Path {
        match self {
            HtmlOutputTarget::Path(path) | HtmlOutputTarget::Temp(path) => path,
        }
    }
}

fn html_output_target(output: Option<PathBuf>) -> Result<HtmlOutputTarget> {
    if let Some(path) = output {
        return Ok(HtmlOutputTarget::Path(path));
    }
    Ok(HtmlOutputTarget::Temp(temp_html_path()?))
}

fn write_html_to_target(html: &str, target: &HtmlOutputTarget) -> Result<()> {
    let path = target.path();
    fs::write(path, html).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_html_to_temp_creates_file() {
        let html = "<html><body>ok</body></html>";
        let target = html_output_target(None).unwrap();
        let path = target.path().to_path_buf();

        write_html_to_target(html, &target).unwrap();

        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, html);

        fs::remove_file(&path).unwrap();
    }
}
