#![warn(clippy::all, clippy::pedantic, clippy::restriction)]
#![allow(
    clippy::missing_docs_in_private_items,
    clippy::implicit_return,
    clippy::shadow_reuse,
    clippy::print_stdout,
    clippy::wildcard_enum_match_arm,
    clippy::else_if_without_else,
)]
mod editor;
mod terminal;
mod row;
mod document;
mod filetype;
mod highlighting;
mod modes; // different modes for Rum

use editor::Editor;
pub use editor::{Pos, SearchDirection};
pub use terminal::Terminal;
pub use row::Row;
pub use filetype::{FileType, HighlightingOptions};

fn main() {
    Editor::default().run();
}
