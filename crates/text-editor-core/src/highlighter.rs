use block_client::{
    blocks::text::{TextDocument, TextLanguage},
    BlockHandle,
};
use tree_sitter::{InputEdit, Parser, Point, Tree};
use tree_sitter_md::{MarkdownParser, MarkdownTree};

mod markdown;
mod rust;
mod zig;

pub use markdown::{MarkdownTable, MarkdownTableAlignment, MarkdownTableRow};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Markdown,
    Rust,
    Zig,
}

impl Language {
    /// The highlighter for a document's stored language, or `None` when that
    /// language is not highlighted at all.
    pub const fn for_document(language: TextLanguage) -> Option<Self> {
        match language {
            TextLanguage::PlainText => None,
            TextLanguage::Markdown => Some(Self::Markdown),
            TextLanguage::Rust => Some(Self::Rust),
            TextLanguage::Zig => Some(Self::Zig),
        }
    }

    fn chain_start_offset(self, kind: &str, source: &[u8]) -> usize {
        match self {
            Self::Markdown => 0,
            Self::Rust => rust::chain_start_offset(kind, source),
            Self::Zig => zig::chain_start_offset(kind, source),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SynHlColorScope {
    Invalid,
    PunctuationImportant,
    Punctuation,
    VariableFunction,
    VariableParameter,
    VariableConstant,
    VariableMutable,
    Variable,
    LiteralString,
    Literal,
    KeywordStorage,
    KeywordPrimitiveType,
    Keyword,
    Comment,
    MarkdownPlainText,
    MarkdownSymbol,
    MarkdownLink,
    MarkdownCode,
    Unstyled,
    Invisible,
}

impl SynHlColorScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invalid => "invalid",
            Self::PunctuationImportant => "punctuation_important",
            Self::Punctuation => "punctuation",
            Self::VariableFunction => "variable_function",
            Self::VariableParameter => "variable_parameter",
            Self::VariableConstant => "variable_constant",
            Self::VariableMutable => "variable_mutable",
            Self::Variable => "variable",
            Self::LiteralString => "literal_string",
            Self::Literal => "literal",
            Self::KeywordStorage => "keyword_storage",
            Self::KeywordPrimitiveType => "keyword_primitive_type",
            Self::Keyword => "keyword",
            Self::Comment => "comment",
            Self::MarkdownPlainText => "markdown_plain_text",
            Self::MarkdownSymbol => "markdown_symbol",
            Self::MarkdownLink => "markdown_link",
            Self::MarkdownCode => "markdown_code",
            Self::Unstyled => "unstyled",
            Self::Invisible => "invisible",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SynHlFontFamily {
    #[default]
    Proportional,
    Monospace,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SynHlTextSize {
    #[default]
    Body,
    Heading(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SynHlStyle {
    pub color: SynHlColorScope,
    pub family: SynHlFontFamily,
    pub size: SynHlTextSize,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

impl SynHlStyle {
    const fn plain(color: SynHlColorScope) -> Self {
        Self {
            color,
            family: SynHlFontFamily::Proportional,
            size: SynHlTextSize::Body,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
        }
    }
}

pub struct SyntaxHighlight {
    styles: Vec<SynHlStyle>,
    markdown_tables: Vec<MarkdownTable>,
}

impl SyntaxHighlight {
    pub(crate) fn plaintext(len: usize) -> Self {
        Self {
            styles: vec![SynHlStyle::plain(SynHlColorScope::Unstyled); len],
            markdown_tables: Vec::new(),
        }
    }

    fn from_scopes(mut scopes: Vec<SynHlColorScope>, bytes: &[u8]) -> Self {
        for index in 1..scopes.len() {
            if bytes[index].is_ascii_whitespace() {
                scopes[index] = scopes[index - 1];
            }
        }
        Self {
            styles: scopes.into_iter().map(SynHlStyle::plain).collect(),
            markdown_tables: Vec::new(),
        }
    }

    pub fn advance_and_read(&self, byte_index: usize) -> SynHlColorScope {
        self.styles
            .get(byte_index)
            .map(|style| style.color)
            .unwrap_or(SynHlColorScope::Invalid)
    }

    pub fn style_at(&self, byte_index: usize) -> SynHlStyle {
        self.styles
            .get(byte_index)
            .copied()
            .unwrap_or(SynHlStyle::plain(SynHlColorScope::Invalid))
    }

    pub fn markdown_tables(&self) -> &[MarkdownTable] {
        &self.markdown_tables
    }
}

enum ParserBackend {
    TreeSitter {
        parser: Parser,
        tree: Option<Tree>,
    },
    Markdown {
        parser: MarkdownParser,
        tree: Option<MarkdownTree>,
    },
}

impl ParserBackend {
    fn tree_sitter(language: &tree_sitter::Language) -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(language)
            .expect("tree-sitter language is incompatible");
        Self::TreeSitter { parser, tree: None }
    }
}

pub struct Highlighter {
    document: BlockHandle<TextDocument>,
    backend: ParserBackend,
    parsed_revision: Option<u64>,
    parsed_bytes: Vec<u8>,
    language: Language,
}

impl Highlighter {
    pub fn new(document: BlockHandle<TextDocument>, language: Language) -> Self {
        let backend = match language {
            Language::Markdown => ParserBackend::Markdown {
                parser: MarkdownParser::default(),
                tree: None,
            },
            Language::Rust => ParserBackend::tree_sitter(&tree_sitter_rust::LANGUAGE.into()),
            Language::Zig => ParserBackend::tree_sitter(&tree_sitter_zig::LANGUAGE.into()),
        };
        Self {
            document,
            backend,
            parsed_revision: None,
            parsed_bytes: Vec::new(),
            language,
        }
    }

    fn ensure_parsed(&mut self) -> Option<()> {
        let revision = self.document.revision();
        let has_tree = match &self.backend {
            ParserBackend::TreeSitter { tree, .. } => tree.is_some(),
            ParserBackend::Markdown { tree, .. } => tree.is_some(),
        };
        if self.parsed_revision == Some(revision) && has_tree {
            return Some(());
        }
        let document = self.document.read()?;
        let bytes = document.bytes();
        let edit = if self.parsed_revision.is_some() {
            let prefix = self
                .parsed_bytes
                .iter()
                .zip(bytes)
                .take_while(|(left, right)| left == right)
                .count();
            let maximum_suffix = self.parsed_bytes.len().min(bytes.len()) - prefix;
            let suffix = self
                .parsed_bytes
                .iter()
                .rev()
                .zip(bytes.iter().rev())
                .take(maximum_suffix)
                .take_while(|(left, right)| left == right)
                .count();
            let old_end = self.parsed_bytes.len() - suffix;
            let new_end = bytes.len() - suffix;
            Some(InputEdit {
                start_byte: prefix,
                old_end_byte: old_end,
                new_end_byte: new_end,
                start_position: byte_point(&self.parsed_bytes, prefix),
                old_end_position: byte_point(&self.parsed_bytes, old_end),
                new_end_position: byte_point(bytes, new_end),
            })
        } else {
            None
        };
        match &mut self.backend {
            ParserBackend::TreeSitter { parser, tree } => {
                if let (Some(tree), Some(edit)) = (tree.as_mut(), edit.as_ref()) {
                    tree.edit(edit);
                }
                *tree = parser.parse(bytes, tree.as_ref());
            }
            ParserBackend::Markdown { parser, tree } => {
                if let (Some(tree), Some(edit)) = (tree.as_mut(), edit.as_ref()) {
                    tree.edit(edit);
                }
                *tree = parser.parse(bytes, tree.as_ref());
            }
        }
        self.parsed_bytes.clear();
        self.parsed_bytes.extend_from_slice(bytes);
        self.parsed_revision = Some(revision);
        Some(())
    }

    pub fn highlight(&mut self) -> SyntaxHighlight {
        if self.ensure_parsed().is_none() {
            return SyntaxHighlight {
                styles: Vec::new(),
                markdown_tables: Vec::new(),
            };
        }
        let document = self.document.read().expect("parsed document disappeared");
        let bytes = document.bytes();
        match self.language {
            Language::Markdown => {
                let (styles, markdown_tables) = match &self.backend {
                    ParserBackend::Markdown {
                        tree: Some(tree), ..
                    } => (markdown::styles(tree, bytes.len()), markdown::tables(tree)),
                    _ => (
                        vec![SynHlStyle::plain(SynHlColorScope::MarkdownPlainText); bytes.len()],
                        Vec::new(),
                    ),
                };
                SyntaxHighlight {
                    styles,
                    markdown_tables,
                }
            }
            Language::Rust => SyntaxHighlight::from_scopes(rust::scopes(bytes), bytes),
            Language::Zig => SyntaxHighlight::from_scopes(zig::scopes(bytes), bytes),
        }
    }

    pub(crate) fn node_chain(&mut self, start: usize, end: usize) -> Vec<(usize, usize)> {
        if self.ensure_parsed().is_none() {
            return Vec::new();
        }
        if let ParserBackend::Markdown {
            tree: Some(tree), ..
        } = &self.backend
        {
            let document_len = tree.block_tree().root_node().end_byte();
            let query_end = if start == end {
                start.saturating_add(1).min(document_len)
            } else {
                end.min(document_len)
            };
            let mut result = Vec::new();
            let mut cursor = tree.walk();
            markdown::collect_chain(&mut cursor, start.min(document_len), query_end, &mut result);
            result.reverse();
            result.dedup();
            return result;
        }
        let ParserBackend::TreeSitter {
            tree: Some(tree), ..
        } = &self.backend
        else {
            return Vec::new();
        };
        let root = tree.root_node();
        let document_len = root.end_byte();
        let bytes = self
            .document
            .read()
            .map(|document| document.bytes().to_vec())
            .unwrap_or_default();
        let query_end = if start == end {
            start.saturating_add(1).min(document_len)
        } else {
            end
        };
        let mut node =
            root.descendant_for_byte_range(start.min(document_len), query_end.min(document_len));
        let mut result = Vec::new();
        while let Some(current) = node {
            let mut range = (current.start_byte(), current.end_byte());
            let source = &bytes[range.0.min(bytes.len())..range.1.min(bytes.len())];
            range.0 += self.language.chain_start_offset(current.kind(), source);
            if result.last().copied() != Some(range) {
                result.push(range);
            }
            node = current.parent();
        }
        result
    }
}

fn byte_point(bytes: &[u8], index: usize) -> Point {
    let index = index.min(bytes.len());
    let row = bytes[..index].iter().filter(|byte| **byte == b'\n').count();
    let column = bytes[..index]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(index, |newline| index - newline - 1);
    Point { row, column }
}

fn identifier_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
        index += 1;
    }
    index
}
