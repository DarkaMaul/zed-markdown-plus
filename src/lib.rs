//! Zed launcher for the separately installed Markdown Folds language server.
#![forbid(unsafe_code)]
#![deny(missing_docs, rustdoc::broken_intra_doc_links)]

use std::path::Path;

use zed_extension_api::{self as zed, settings::LspSettings};

struct MarkdownFoldsExtension;

impl zed::Extension for MarkdownFoldsExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        if language_server_id.as_ref() != "markdown-folds" {
            return Err(format!("Unsupported language server: {language_server_id}"));
        }

        let settings = LspSettings::for_worktree("markdown-folds", worktree)?;
        let path = settings
            .binary
            .and_then(|binary| binary.path)
            .ok_or_else(|| {
                "Configure lsp.markdown-folds.binary.path with the absolute path to the locally installed markdown-folds-server executable".to_string()
            })?;
        if !Path::new(&path).is_absolute() {
            return Err("lsp.markdown-folds.binary.path must be an absolute path".to_string());
        }

        Ok(zed::Command {
            command: path,
            args: Vec::new(),
            env: Vec::new(),
        })
    }
}

// The SDK generates undocumented ABI entry points, not a public Rust API.
#[allow(missing_docs)]
mod registration {
    zed_extension_api::register_extension!(super::MarkdownFoldsExtension);
}
