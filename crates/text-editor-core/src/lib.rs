mod core;
mod document;
mod highlighter;

pub use core::*;
pub use document::{
    Anchor, Document, DocumentEdit, DocumentRead, DocumentView, TextBuffer, TextIndentation,
    TextLanguage,
};
pub use highlighter::{
    Highlighter, Language, MarkdownTable, MarkdownTableAlignment, MarkdownTableRow,
    SynHlColorScope, SynHlFontFamily, SynHlStyle, SynHlTextSize, SyntaxHighlight,
};

#[cfg(test)]
mod tests;
