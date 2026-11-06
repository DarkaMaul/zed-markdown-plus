# Releasing

Target repository: `DarkaMaul/zed-markdown-plus`. This document describes the
process; no repository, public release, or registry submission is implied by its
presence. `0.1.0` is the first planned release.

## Before the first release

- Create the GitHub repository and push a reviewed source-only initial commit.
  Exclude build outputs, private documents, local settings, credentials, and
  editor logs. This project started without Git history.
- Enable branch protection for `main`, require pull requests and both CI matrix
  checks, and disallow force pushes. Enable Dependabot alerts/security updates
  and private vulnerability reporting. Dependabot update configuration is checked in.
- Review LICENSE and NOTICE. Apache-2.0 is the chosen project license.
- Confirm the README's build and required settings work on a clean checkout.
  Installing the extension alone is insufficient: users need the separate server
  executable and its absolute path in Zed settings.
- Run all checks in CONTRIBUTING.md. `cargo deny check advisories sources`
  provides an additional dependency audit when cargo-deny is installed.
- Inspect the packaged archives and verify their checksums. macOS artifacts are
  not notarized or Developer ID signed. Do not present them as notarized.

## Tag and draft

The root crate, server crate, and extension manifest must have identical versions.
`release.toml` configures cargo-release to synchronize Rust versions and update
`extension.toml`; it neither publishes crates nor pushes changes automatically.

1. Update CHANGELOG.md with the intended version and release date.
2. Run `cargo release --workspace` for a dry run of the current version, or
   `cargo release patch --workspace` for the next patch. Review the proposed changes.
3. Run the corresponding command with `--execute` when ready to commit and tag.
   Inspect the resulting commit and `vX.Y.Z` tag before pushing them.
4. Push the release commit and tag. Tag pushes run CI and create a **draft** GitHub
   release with separately packaged macOS arm64 and Linux x86_64 native servers.
5. Review the draft release and checksums before publishing it. A failed build
   must be fixed before publishing; do not mark an untested platform as supported.

To test the packaging locally without creating a release or tag:

```sh
bash scripts/package-release.sh package v0.1.0 macos-aarch64
```

Use `linux-x86_64` on the matching Linux host. The archive includes the server,
README, and project license/notice. The Zed
extension itself does not bundle the native server. Zed builds the WASM extension
from the published source revision.

## Zed registry submission

Follow Zed's [publishing prerequisites](https://zed.dev/docs/extensions/publishing/prerequisites)
and [publishing guide](https://zed.dev/docs/extensions/publishing/publishing-guide).

- Test the exact commit being submitted in Zed: section folds at every heading
  level, setext headings, Java/Python fences, unknown labels, edits, and restart.
  Use `examples/demo.md`; keep private documents out of screenshots.
- Use extension ID `markdown-folds-lsp` and repository URL from `extension.toml`.
  The existing LSP setting stays `markdown-folds`.
- Explain the distinction from `custom-code-folding-lsp`: this server folds
  CommonMark heading hierarchies and adds local lexer-based fenced-code colors,
  rather than configurable comment-marker folds. Registry acceptance is subject
  to the maintainers' assessment of overlap with existing extensions.
- Fork `zed-industries/extensions` to a personal account, add this repository as
  one extension submodule, and submit one extension per PR using `gh`.
- Do not claim registry availability until the submission is accepted.

## Release evidence to record

Record the tested commit, CI run, local OS/Zed version, dependency audit result,
archive hashes, and manual smoke-test results in the release notes or PR.
Local checks cannot replace the first hosted Linux/macOS CI runs. Coverage
regression gating is not configured; behavioral and protocol tests are required
instead. There is no stable public Rust library API or separate documentation site.
