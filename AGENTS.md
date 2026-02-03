# Repository Guidelines

## Project Structure & Module Organization

- `pyimporttime/` is the Rust crate root (run all cargo commands from here).
- `pyimporttime/src/main.rs` wires the CLI to the internal modules.
- Core modules live in `pyimporttime/src/` (e.g., `cli.rs`, `parser.rs`, `layout.rs`, `render.rs`, `tree.rs`, `util.rs`).
- The repository root contains top-level docs like `README.md`.

## Build, Test, and Development Commands

- `cargo build` (run in `pyimporttime/`): compile the CLI binary.
- `cargo run -- run -- python your_script.py`: run the profiler and generate HTML (default opens a browser).
- `cargo run -- run --open=false -- python your_script.py`: generate HTML without opening a browser.
- `cargo run -- parse import-times.txt`: parse a saved import log for inspection.
- `cargo run -- graph import-times.txt -o /tmp/pyimporttime.html`: generate HTML from a saved log.

## Coding Style & Naming Conventions

- Follow standard Rust style (4-space indentation, `snake_case` for functions/modules, `CamelCase` for types).
- Keep module boundaries clear: parsing in `parser.rs`, layout in `layout.rs`, rendering in `render.rs`.
- Prefer small, focused helpers in `util.rs` over duplicating logic.

## Testing Guidelines

- There are no automated tests in the repository yet.
- If you add tests, use Rust’s built-in test framework and run them with `cargo test`.
- Name tests after behaviors (e.g., `parses_multiline_import_log`).

## Commit & Pull Request Guidelines

- Commit messages use short, imperative summaries (e.g., “Refactor CLI into modules”).
- PRs should describe the behavior change, list commands run (if any), and note output artifacts (e.g., sample HTML path).
- Include repro steps for CLI changes, ideally with a minimal Python script.

## Security & Configuration Tips

- The tool consumes `PYTHONPROFILEIMPORTTIME=1` stderr output; avoid committing any sensitive logs.
- When sharing generated HTML, confirm it does not embed private paths or identifiers.
