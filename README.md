# pyimporttime

CLI to visualize Python import-time profiling output (from `PYTHONPROFILEIMPORTTIME=1`).

## Quick start

```bash
cargo run -- run --open -- python your_script.py
```

Generate HTML without opening a browser:

```bash
cargo run -- run -o /tmp/pyimporttime.html -- python your_script.py
```

Parse and inspect the raw import-time log:

```bash
PYTHONPROFILEIMPORTTIME=1 python your_script.py 2> import-times.txt
cargo run -- parse import-times.txt
```

Generate HTML from a saved log:

```bash
cargo run -- graph import-times.txt -o /tmp/pyimporttime.html
```

## Attribution

This tool is inspired by and based on the visualization approach from:
- https://github.com/kmichel/python-importtime-graph

The original project uses D3 for client-side layout. This Rust CLI precomputes layout and emits a self-contained HTML/SVG for faster rendering.
