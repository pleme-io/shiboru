//! Shiboru (絞る) — fuzzy finder for files, grep, LSP symbols, and more in Neovim
//!
//! Part of the blnvim-ng distribution — a Rust-native Neovim plugin suite.
//! Built with [`nvim-oxi`](https://github.com/noib3/nvim-oxi) for zero-cost
//! Neovim API bindings.

use nvim_oxi as oxi;

#[oxi::plugin]
fn shiboru() -> oxi::Result<()> {
    Ok(())
}
