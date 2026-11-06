# Changelog

## 0.1.0 — unreleased

- Fold Markdown sections by heading level, including nested sections and setext headings.
- Highlight supported top-level fenced languages using bundled Syntect lexers,
  without requiring language-specific Zed extensions.
- Keep Markdown content inert; bound inputs and isolate lexer work behind a timeout.
- Provide a separate local stdio server and a small Zed launcher.

The release extension ID is `markdown-folds-lsp`. The prototype used `markdown-folds`;
remove that development extension before installing the release extension.
The server ID and settings key remain `markdown-folds` for compatibility.
