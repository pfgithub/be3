mod core;
mod highlighter;

pub use core::*;
pub use highlighter::{
    Highlighter, Language, MarkdownTable, MarkdownTableAlignment, MarkdownTableRow,
    SynHlColorScope, SynHlFontFamily, SynHlStyle, SynHlTextSize, SyntaxHighlight,
};

#[cfg(test)]
mod tests;
