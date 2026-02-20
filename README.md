# md_to_html

A fast Markdown to HTML converter with live reload, written in Rust.
(Note: only a part of Markdown is implemented for now)

## Features

- Converts Markdown to HTML instantly
- Live reload — edit your file and the browser updates automatically
- Embedded HTTP server, no external tools needed
- Custom CSS support via config file
- Built-in dark theme

## Supported Markdown Syntax

| Markdown | Output |
|---|---|
| `# Heading` through `###### Heading` | `<h1>` through `<h6>` |
| `**bold**` | **bold** |
| `*italic*` | *italic* |
| `` `inline code` `` | `inline code` |
| `- item` | Unordered list |
| `1. item` | Ordered list |
| `[text](url)` | Link |
| `---` | Horizontal rule |
| ` ``` ` | Code block |

## Installation

Clone the repository and build with Cargo:

```
git clone https://github.com/yourname/md_to_html
cd md_to_html
cargo build --release
```

The binary will be at `target/release/md_to_html`.

## Usage

```
md_to_html <input.md> <output.html> [OPTIONS]
```

### Options

| Flag | Description |
|---|---|
| `-w, --watch` | Watch the input file and reload the browser on changes |
| `-c, --config <file>` | Path to a custom CSS file |

### Examples

Basic conversion:
```
md_to_html README.md index.html
```

With live reload:
```
md_to_html README.md index.html --watch
```

With a custom stylesheet:
```
md_to_html README.md index.html --watch --config my_style.css
```

When `--watch` is used, the converter starts a local server at `http://localhost:8080` and opens it in your default browser automatically.

## Performance

Benchmarked on a 2.2MB file (100,000 lines):

| Lines | File size | Mean time |
|---|---|---|
| 1,000 | 21 KB | ~12ms |
| 10,000 | 218 KB | ~20ms |
| 100,000 | 2.2 MB | ~142ms |
| 500,000 | 11.4 MB | ~660ms |

## License

MIT