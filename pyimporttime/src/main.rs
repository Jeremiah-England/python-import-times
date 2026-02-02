mod cli;
mod layout;
mod parser;
mod render;
mod tree;
mod util;

fn main() -> anyhow::Result<()> {
    cli::run()
}
