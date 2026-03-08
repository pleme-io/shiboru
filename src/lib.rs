//! Shiboru (絞る) — fuzzy finder for files, grep, LSP symbols, and more in Neovim
//!
//! Part of the blnvim-ng distribution — a Rust-native Neovim plugin suite.
//! Built with [`nvim-oxi`](https://github.com/noib3/nvim-oxi) for zero-cost
//! Neovim API bindings.
//!
//! # Commands
//!
//! - `:ShiboruFiles` — find files in the working directory
//! - `:ShiboruGrep`  — live grep across file contents
//! - `:ShiboruBuffers` — switch between open buffers

pub mod picker;
pub mod sources;
pub mod ui;

use nvim_oxi as oxi;
use sources::buffers::BufferSource;
use sources::files::FileSource;
use sources::grep::GrepSource;
use tane::usercmd::UserCommand;
use ui::PickerUi;

/// Get the current working directory from Neovim.
fn cwd() -> std::path::PathBuf {
    oxi::api::eval::<String>("getcwd()")
        .map(|s| std::path::PathBuf::from(s.trim()))
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| ".".into()))
}

#[oxi::plugin]
fn shiboru() -> oxi::Result<()> {
    // Register :ShiboruFiles
    UserCommand::new("ShiboruFiles")
        .desc("Shiboru: find files")
        .register(|_args| {
            let source = FileSource::new(cwd());
            let mut picker_ui =
                PickerUi::new().map_err(|e| tane::Error::Custom(e.to_string()))?;
            picker_ui
                .open(source)
                .map_err(|e| tane::Error::Custom(e.to_string()))?;
            Ok(())
        })
        .map_err(|e| oxi::api::Error::Other(e.to_string()))?;

    // Register :ShiboruGrep
    UserCommand::new("ShiboruGrep")
        .desc("Shiboru: live grep")
        .register(|_args| {
            let source = GrepSource::new(cwd());
            let mut picker_ui =
                PickerUi::new().map_err(|e| tane::Error::Custom(e.to_string()))?;
            picker_ui
                .open(source)
                .map_err(|e| tane::Error::Custom(e.to_string()))?;
            Ok(())
        })
        .map_err(|e| oxi::api::Error::Other(e.to_string()))?;

    // Register :ShiboruBuffers
    UserCommand::new("ShiboruBuffers")
        .desc("Shiboru: switch buffer")
        .register(|_args| {
            let source = BufferSource::new();
            let mut picker_ui =
                PickerUi::new().map_err(|e| tane::Error::Custom(e.to_string()))?;
            picker_ui
                .open(source)
                .map_err(|e| tane::Error::Custom(e.to_string()))?;
            Ok(())
        })
        .map_err(|e| oxi::api::Error::Other(e.to_string()))?;

    Ok(())
}
