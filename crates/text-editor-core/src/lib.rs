mod core;
mod highlighter;
mod presence;

pub use core::*;
pub use highlighter::{
    Highlighter, Language, MarkdownTable, MarkdownTableAlignment, MarkdownTableRow,
    SynHlColorScope, SynHlFontFamily, SynHlStyle, SynHlTextSize, SyntaxHighlight,
};
pub use presence::TextCursor;

#[cfg(test)]
mod tests;
