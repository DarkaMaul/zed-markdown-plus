# Markdown Folds for Zed

A small local language server adds heading-section folding and fenced-code colors
to Zed's built-in Markdown support. No language-specific extension is required for
languages included in Syntect's bundled syntax set, including Java, Python, and
JavaScript. Unknown or unlabeled fences remain plain text.

## Install

Installation has two parts: the Zed extension and the native server. The extension
does not download or build the server automatically. You must configure the
absolute server path below before it can start.

Use Zed with support for LSP folding ranges and semantic tokens; tested locally
with Zed 1.27.0 on macOS Apple Silicon. Release builds target macOS Apple Silicon
and Linux x86_64 (built on Ubuntu 26.04). Older Linux distributions and other
platforms are not validated.

### Native server

After a release is published, download the archive for your platform and its
`.sha256` file from this repository's GitHub Releases. Verify the checksum before
extracting and copy `markdown-folds-server` to a trusted permanent location.
Retain the accompanying license files. Checksums detect corruption, not a
compromised release account.

Alternatively, build from source with Rust via rustup:

```sh
cargo build --release -p markdown-folds-server --locked
```

The executable is `target/release/markdown-folds-server`. Its parent directory must
remain available for as long as Zed uses that path.

### Zed extension

Until the extension is accepted into Zed's registry, install it as a development
extension from this source directory. To check the WebAssembly build yourself:

```sh
cargo build --release -p markdown-folds --target wasm32-wasip2 --locked
```

In Zed, run **zed: install dev extension** and select this directory. Zed builds the
extension; its release ID is `markdown-folds-lsp`. If you installed the earlier
prototype ID `markdown-folds`, remove that development extension first.

### Required settings

Merge these entries into your Zed user settings, replacing the server path with
the absolute path to your build (or a trusted location where you copied it):

```json
{
  "lsp": {
    "markdown-folds": {
      "binary": {
        "path": "/absolute/path/to/markdown-folds-server"
      }
    }
  },
  "languages": {
    "Markdown": {
      "language_servers": ["markdown-folds", "..."],
      "document_folding_ranges": "on",
      "semantic_tokens": "combined"
    }
  }
}
```

Reopen a Markdown file or run **editor: restart language server**. Use the fold
arrow beside a heading to collapse its section, including lower-level headings,
until the next heading of equal or higher level. ATX (`#`) and setext (underlined)
headings work. Headings inside code, lists, and blockquotes do not start document
sections. See [examples/demo.md](examples/demo.md).

Highlighting is lexical, not compiler analysis, and uses the first word of a
fence's language label. It covers top-level fences only; fenced blocks nested in
lists or blockquotes retain Zed's existing highlighting. Semantic colors overlay
Zed's normal colors and use your theme. Disable just the extra lexer by setting
Markdown's `semantic_tokens` to `"off"`.

## Security and limits

Markdown is data. The server never follows document URIs, reads referenced files,
renders HTML, executes fenced code, loads user lexer definitions, or uses the
network. Dependencies are fetched only when building. The sole subprocess is a
copy of the server itself with a fixed worker argument and text over stdin; no
shell is used. Each highlighting request has a two-second deadline, after which
the worker is killed and reaped and no additional colors are returned.

Documents are limited to 2 MiB, total retained text to 16 MiB / 256 documents, and
LSP frames to 8 MiB. Highlighting has additional line, byte, and token limits; it
may stop early on large blocks. These bounds and the worker timeout mitigate
resource exhaustion; they are not an OS sandbox or a hard process-memory quota.
Zed settings and the configured executable must be trusted, including project
settings that can override the server command.

## Verify

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --release -p markdown-folds-server --locked
python3 tests/protocol.py
```

The code is split between the small Zed launcher (`src/lib.rs`), LSP and section
folds (`server/src/main.rs`), lexer (`highlight.rs`), and timeout worker (`worker.rs`).
No Markdown grammar fork, automatic downloads, or per-language server setup.

## Contributing and releases

See [CONTRIBUTING.md](CONTRIBUTING.md) for development checks,
[SECURITY.md](SECURITY.md) for private reporting, and [RELEASING.md](RELEASING.md)
for the release checklist. Changes are recorded in [CHANGELOG.md](CHANGELOG.md).

## License

Apache-2.0; see [LICENSE](LICENSE) and [NOTICE](NOTICE).
