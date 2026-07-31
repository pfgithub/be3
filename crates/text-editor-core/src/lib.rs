mod core;
mod highlighter;

pub use core::*;
pub use highlighter::{Highlighter, Language, SynHlColorScope, SyntaxHighlight};

#[cfg(test)]
mod tests;
