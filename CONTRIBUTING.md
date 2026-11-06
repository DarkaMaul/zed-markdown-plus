# Contributing

Small bug fixes and focused improvements are welcome. For larger changes, open an
issue first so we can keep the extension small and easy to review.

Use the Rust toolchain in `rust-toolchain.toml` and Python 3.11+ for the stdio
integration test. From the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release -p markdown-folds-server --locked
python3 tests/protocol.py
cargo doc --workspace --no-deps --locked
cargo build --release -p markdown-folds --target wasm32-wasip2 --locked
```

Include a regression test for behavior changes. Test inputs must be synthetic;
do not include private reports, credentials, or client data. Markdown, fence
labels, and document URIs are untrusted input. Preserve input limits and the
killable lexer worker. No runtime downloads or execution of document content.

Keep formatting-only changes separate from behavior changes. CI checks Linux
x86_64 and macOS Apple Silicon; run the manual demo in Zed before a release.
The Rust crates are implementation details, not a stable public library API.
`cargo doc` builds local implementation documentation; no hosted API site is needed.

By contributing, you agree to license your contributions under Apache-2.0.
