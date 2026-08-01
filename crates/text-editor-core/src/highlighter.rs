use block_client::{blocks::text::TextDocument, BlockHandle};
use std::ops::Range;
use tree_sitter::{InputEdit, Node, Parser, Point, Tree};
use tree_sitter_md::{MarkdownCursor, MarkdownParser, MarkdownTree};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Markdown,
    Zig,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownTableAlignment {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownTableRow {
    pub range: Range<usize>,
    pub cells: Vec<Range<usize>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownTable {
    pub rows: Vec<MarkdownTableRow>,
    pub alignments: Vec<MarkdownTableAlignment>,
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
    Zig {
        parser: Parser,
        tree: Option<Tree>,
    },
    Markdown {
        parser: MarkdownParser,
        tree: Option<MarkdownTree>,
    },
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
            Language::Zig => {
                let mut parser = Parser::new();
                parser
                    .set_language(&tree_sitter_zig::LANGUAGE.into())
                    .expect("tree-sitter Zig language is incompatible");
                ParserBackend::Zig { parser, tree: None }
            }
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
            ParserBackend::Zig { tree, .. } => tree.is_some(),
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
            ParserBackend::Zig { parser, tree } => {
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
        if self.language == Language::Markdown {
            let (styles, markdown_tables) = match &self.backend {
                ParserBackend::Markdown {
                    tree: Some(tree), ..
                } => (markdown_styles(tree, bytes.len()), markdown_tables(tree)),
                _ => (
                    vec![SynHlStyle::plain(SynHlColorScope::MarkdownPlainText); bytes.len()],
                    Vec::new(),
                ),
            };
            return SyntaxHighlight {
                styles,
                markdown_tables,
            };
        }
        let mut scopes = zig_scopes(bytes);
        for index in 1..scopes.len() {
            if bytes[index].is_ascii_whitespace() {
                scopes[index] = scopes[index - 1];
            }
        }
        SyntaxHighlight {
            styles: scopes.into_iter().map(SynHlStyle::plain).collect(),
            markdown_tables: Vec::new(),
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
            collect_markdown_chain(&mut cursor, start.min(document_len), query_end, &mut result);
            result.reverse();
            result.dedup();
            return result;
        }
        let ParserBackend::Zig {
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
            if current.kind().contains("function_declaration") {
                let source = &bytes[range.0.min(bytes.len())..range.1.min(bytes.len())];
                if source.starts_with(b"pub ") {
                    range.0 += 4;
                }
            }
            if result.last().copied() != Some(range) {
                result.push(range);
            }
            node = current.parent();
        }
        result
    }
}

fn markdown_styles(tree: &MarkdownTree, len: usize) -> Vec<SynHlStyle> {
    let mut styles = vec![SynHlStyle::plain(SynHlColorScope::MarkdownPlainText); len];
    let mut cursor = tree.walk();
    style_markdown_node(&mut cursor, &mut styles);
    styles
}

fn markdown_tables(tree: &MarkdownTree) -> Vec<MarkdownTable> {
    let mut tables = Vec::new();
    let mut cursor = tree.walk();
    collect_markdown_tables(&mut cursor, &mut tables);
    tables
}

fn collect_markdown_tables(cursor: &mut MarkdownCursor<'_>, tables: &mut Vec<MarkdownTable>) {
    if cursor.node().kind() == "pipe_table" {
        tables.push(markdown_table(cursor));
        return;
    }
    if cursor.goto_first_child() {
        loop {
            collect_markdown_tables(cursor, tables);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn markdown_table(cursor: &mut MarkdownCursor<'_>) -> MarkdownTable {
    let mut rows = Vec::new();
    let mut alignments = Vec::new();
    if cursor.goto_first_child() {
        loop {
            let kind = cursor.node().kind();
            if matches!(
                kind,
                "pipe_table_header" | "pipe_table_delimiter_row" | "pipe_table_row"
            ) {
                let delimiter = kind == "pipe_table_delimiter_row";
                let row = markdown_table_row(cursor, delimiter, &mut alignments);
                rows.push(row);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    MarkdownTable { rows, alignments }
}

fn markdown_table_row(
    cursor: &mut MarkdownCursor<'_>,
    delimiter: bool,
    alignments: &mut Vec<MarkdownTableAlignment>,
) -> MarkdownTableRow {
    let node = cursor.node();
    let range = node.start_byte()..node.end_byte();
    let mut cells = Vec::new();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if matches!(node.kind(), "pipe_table_cell" | "pipe_table_delimiter_cell") {
                cells.push(node.start_byte()..node.end_byte());
                if delimiter {
                    alignments.push(markdown_table_alignment(cursor));
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    MarkdownTableRow { range, cells }
}

fn markdown_table_alignment(cursor: &mut MarkdownCursor<'_>) -> MarkdownTableAlignment {
    let mut left = false;
    let mut right = false;
    if cursor.goto_first_child() {
        loop {
            match cursor.node().kind() {
                "pipe_table_align_left" => left = true,
                "pipe_table_align_right" => right = true,
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    match (left, right) {
        (true, true) => MarkdownTableAlignment::Center,
        (_, true) => MarkdownTableAlignment::Right,
        _ => MarkdownTableAlignment::Left,
    }
}

fn style_markdown_node(cursor: &mut MarkdownCursor<'_>, styles: &mut [SynHlStyle]) {
    let node = cursor.node();
    let range = node.start_byte().min(styles.len())..node.end_byte().min(styles.len());
    let kind = node.kind();
    let parent_kind = node.parent().map(|parent| parent.kind());

    let contextual_symbol = match kind {
        "[" | "]" => parent_kind.is_some_and(|parent| {
            matches!(
                parent,
                "image"
                    | "inline_link"
                    | "shortcut_link"
                    | "collapsed_reference_link"
                    | "full_reference_link"
            )
        }),
        "(" | ")" => parent_kind.is_some_and(|parent| matches!(parent, "image" | "inline_link")),
        "!" => parent_kind == Some("image"),
        "|" => parent_kind.is_some_and(|parent| parent.starts_with("pipe_table")),
        "~" => parent_kind == Some("strikethrough"),
        _ => false,
    };
    if contextual_symbol {
        for style in &mut styles[range.clone()] {
            set_markdown_symbol(style);
        }
    }

    match kind {
        "strong_emphasis" => {
            for style in &mut styles[range.clone()] {
                style.bold = true;
            }
        }
        "emphasis" => {
            for style in &mut styles[range.clone()] {
                style.italic = true;
            }
        }
        "strikethrough" => {
            for style in &mut styles[range.clone()] {
                style.strikethrough = true;
            }
        }
        "atx_heading" => {
            let level = node
                .child(0)
                .and_then(|marker| marker.kind().strip_prefix("atx_h"))
                .and_then(|value| value.strip_suffix("_marker"))
                .and_then(|value| value.parse().ok())
                .unwrap_or(6);
            set_heading(&mut styles[range.clone()], level);
        }
        "setext_heading" => {
            let mut child = node.walk();
            let level = node
                .children(&mut child)
                .find_map(|child| match child.kind() {
                    "setext_h1_underline" => Some(1),
                    "setext_h2_underline" => Some(2),
                    _ => None,
                })
                .unwrap_or(2);
            set_heading(&mut styles[range.clone()], level);
        }
        "block_quote" => {
            for style in &mut styles[range.clone()] {
                style.italic = true;
            }
        }
        "pipe_table_header" => {
            for style in &mut styles[range.clone()] {
                style.bold = true;
            }
        }
        "code_span" | "indented_code_block" | "code_fence_content" => {
            for style in &mut styles[range.clone()] {
                style.color = SynHlColorScope::MarkdownCode;
                style.family = SynHlFontFamily::Monospace;
                style.size = SynHlTextSize::Body;
                style.bold = false;
                style.italic = false;
            }
        }
        "info_string" => {
            for style in &mut styles[range.clone()] {
                style.color = SynHlColorScope::MarkdownLink;
                style.family = SynHlFontFamily::Monospace;
            }
        }
        "link_text" | "link_label" | "image_description" | "uri_autolink" | "email_autolink" => {
            for style in &mut styles[range.clone()] {
                style.color = SynHlColorScope::MarkdownLink;
                style.underline = true;
            }
        }
        "link_destination" | "link_title" => {
            for style in &mut styles[range.clone()] {
                style.color = SynHlColorScope::MarkdownLink;
                style.family = SynHlFontFamily::Monospace;
            }
        }
        "backslash_escape" => {
            if let Some(style) = styles.get_mut(range.start) {
                set_markdown_symbol(style);
            }
        }
        "emphasis_delimiter"
        | "code_span_delimiter"
        | "fenced_code_block_delimiter"
        | "block_quote_marker"
        | "block_continuation"
        | "list_marker_plus"
        | "list_marker_minus"
        | "list_marker_star"
        | "list_marker_dot"
        | "list_marker_parenthesis"
        | "task_list_marker_checked"
        | "task_list_marker_unchecked"
        | "thematic_break"
        | "setext_h1_underline"
        | "setext_h2_underline"
        | "pipe_table_delimiter_row"
        | "pipe_table_delimiter_cell"
        | "pipe_table_align_left"
        | "pipe_table_align_right"
        | "hard_line_break"
        | "atx_h1_marker"
        | "atx_h2_marker"
        | "atx_h3_marker"
        | "atx_h4_marker"
        | "atx_h5_marker"
        | "atx_h6_marker" => {
            for style in &mut styles[range.clone()] {
                set_markdown_symbol(style);
            }
        }
        _ => {}
    }

    if cursor.goto_first_child() {
        loop {
            style_markdown_node(cursor, styles);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn set_heading(styles: &mut [SynHlStyle], level: u8) {
    for style in styles {
        style.bold = true;
        style.size = SynHlTextSize::Heading(level.clamp(1, 6));
    }
}

fn set_markdown_symbol(style: &mut SynHlStyle) {
    *style = SynHlStyle::plain(SynHlColorScope::MarkdownSymbol);
}

fn collect_markdown_chain(
    cursor: &mut MarkdownCursor<'_>,
    start: usize,
    end: usize,
    result: &mut Vec<(usize, usize)>,
) {
    let node = cursor.node();
    if node.start_byte() > start || node.end_byte() < end {
        return;
    }
    result.push((node.start_byte(), node.end_byte()));
    if cursor.goto_first_child() {
        loop {
            collect_markdown_chain(cursor, start, end, result);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
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

fn zig_scopes(bytes: &[u8]) -> Vec<SynHlColorScope> {
    use SynHlColorScope as Scope;

    let mut scopes = vec![Scope::Invalid; bytes.len()];
    let mut index = 0;
    let mut declaration: Option<Scope> = None;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            scopes[index] = if index == 0 {
                Scope::Unstyled
            } else {
                scopes[index - 1]
            };
            index += 1;
            continue;
        }
        if bytes[index..].starts_with(b"//") {
            let end = bytes[index..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |offset| index + offset);
            let doc =
                bytes[index..end].starts_with(b"///") || bytes[index..end].starts_with(b"//!");
            let punctuation_len = if doc { 3 } else { 2 };
            for scope in &mut scopes[index..(index + punctuation_len).min(end)] {
                *scope = if doc {
                    Scope::Keyword
                } else {
                    Scope::Punctuation
                };
            }
            for scope in &mut scopes[(index + punctuation_len).min(end)..end] {
                *scope = if doc {
                    Scope::MarkdownPlainText
                } else {
                    Scope::Comment
                };
            }
            index = end;
            continue;
        }
        if bytes[index] == b'"' {
            scopes[index] = Scope::Punctuation;
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'"' {
                    scopes[index] = Scope::Punctuation;
                    index += 1;
                    break;
                }
                if bytes[index] == b'\\' {
                    scopes[index] = Scope::Punctuation;
                    index += 1;
                    if index >= bytes.len() {
                        break;
                    }
                    scopes[index] = match bytes[index] {
                        b'n' | b'r' | b't' => Scope::Literal,
                        b'\\' | b'\'' | b'"' => Scope::LiteralString,
                        b'x' | b'u' => Scope::KeywordStorage,
                        _ => Scope::Invalid,
                    };
                    let escape = bytes[index];
                    index += 1;
                    if escape == b'x' {
                        for _ in 0..2 {
                            if index < bytes.len() {
                                scopes[index] = Scope::Literal;
                                index += 1;
                            }
                        }
                    } else if escape == b'u' && bytes.get(index) == Some(&b'{') {
                        scopes[index] = Scope::Punctuation;
                        index += 1;
                        while index < bytes.len() && bytes[index] != b'}' {
                            scopes[index] = Scope::Literal;
                            index += 1;
                        }
                        if index < bytes.len() {
                            scopes[index] = Scope::Punctuation;
                            index += 1;
                        }
                    }
                    continue;
                }
                if bytes[index] == b'{' {
                    scopes[index] = Scope::Punctuation;
                    index += 1;
                    if bytes.get(index) == Some(&b'{') {
                        scopes[index] = Scope::LiteralString;
                        index += 1;
                    }
                    continue;
                }
                if bytes[index] == b'}' {
                    if bytes.get(index + 1) == Some(&b'}') {
                        scopes[index] = Scope::LiteralString;
                        scopes[index + 1] = Scope::Punctuation;
                        index += 2;
                    } else {
                        scopes[index] = Scope::Punctuation;
                        index += 1;
                    }
                    continue;
                }
                scopes[index] = Scope::LiteralString;
                index += 1;
            }
            continue;
        }
        if bytes[index] == b'@' {
            let end = identifier_end(bytes, index + 1);
            scopes[index..end].fill(Scope::Keyword);
            index = end;
            continue;
        }
        if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' {
            let end = identifier_end(bytes, index);
            let token = std::str::from_utf8(&bytes[index..end]).unwrap_or_default();
            let scope = if matches!(token, "const" | "var" | "fn") {
                declaration = Some(match token {
                    "var" => Scope::PunctuationImportant,
                    "fn" => Scope::VariableFunction,
                    _ => Scope::VariableConstant,
                });
                Scope::KeywordStorage
            } else if let Some(scope) = declaration.take() {
                scope
            } else if matches!(token, "true" | "false" | "null" | "undefined") {
                Scope::Literal
            } else if is_primitive(token) {
                Scope::KeywordPrimitiveType
            } else if is_keyword(token) {
                Scope::Keyword
            } else {
                Scope::Variable
            };
            scopes[index..end].fill(scope);
            index = end;
            continue;
        }
        if bytes[index].is_ascii_digit() {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_hexdigit()
                    || matches!(bytes[index], b'x' | b'o' | b'b' | b'_' | b'.'))
            {
                index += 1;
            }
            scopes[start..index].fill(Scope::Literal);
            if index - start >= 2
                && bytes[start] == b'0'
                && matches!(bytes[start + 1], b'x' | b'o' | b'b')
            {
                scopes[start..start + 2].fill(Scope::KeywordStorage);
            }
            continue;
        }
        scopes[index] = if matches!(
            bytes[index],
            b'\'' | b'{' | b'}' | b'(' | b')' | b'[' | b']' | b';' | b',' | b'\\'
        ) {
            Scope::Punctuation
        } else if matches!(bytes[index], b'.' | b'|') {
            Scope::PunctuationImportant
        } else {
            Scope::Keyword
        };
        index += 1;
    }
    scopes
}

fn identifier_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
        index += 1;
    }
    index
}

#[allow(dead_code)]
fn zig_scope(node: Node<'_>, bytes: &[u8], byte_index: usize) -> SynHlColorScope {
    use SynHlColorScope as Scope;

    let kind = node.kind();
    let start = node.start_byte();
    let offset = byte_index.saturating_sub(start);
    let byte = bytes.get(byte_index).copied().unwrap_or_default();

    if matches!(
        kind,
        "line_comment" | "doc_comment" | "container_doc_comment"
    ) {
        let doc = bytes.get(start..start.saturating_add(3)) == Some(b"///")
            || bytes.get(start..start.saturating_add(3)) == Some(b"//!");
        return if doc {
            if offset < 3 {
                Scope::Keyword
            } else {
                Scope::MarkdownPlainText
            }
        } else if offset < 2 {
            Scope::Punctuation
        } else {
            Scope::Comment
        };
    }

    if matches!(kind, "escape_sequence" | "EscapeSequence") {
        return match offset {
            0 => Scope::Punctuation,
            1 => match byte {
                b'n' | b'r' | b't' => Scope::Literal,
                b'\\' | b'\'' | b'"' => Scope::LiteralString,
                b'x' | b'u' => Scope::KeywordStorage,
                _ => Scope::Invalid,
            },
            _ if matches!(byte, b'{' | b'}') => Scope::Punctuation,
            _ => Scope::Literal,
        };
    }

    if kind.contains("string") || kind.contains("String") {
        return if matches!(byte, b'"' | b'\'') {
            Scope::Punctuation
        } else {
            Scope::LiteralString
        };
    }

    if matches!(kind, "integer" | "float" | "INTEGER" | "FLOAT") {
        return if offset < 2
            && bytes.get(start) == Some(&b'0')
            && matches!(bytes.get(start + 1), Some(b'x' | b'o' | b'b'))
        {
            Scope::KeywordStorage
        } else {
            Scope::Literal
        };
    }

    if matches!(kind, "identifier" | "IDENTIFIER") {
        return identifier_scope(node, bytes);
    }

    if matches!(kind, "const" | "var" | "fn") {
        return Scope::KeywordStorage;
    }
    if matches!(kind, "true" | "false" | "null" | "undefined") {
        return Scope::Literal;
    }
    if is_primitive(kind) {
        return Scope::KeywordPrimitiveType;
    }
    if is_keyword(kind) || kind == "builtin_identifier" || kind == "BUILTINIDENTIFIER" {
        return Scope::Keyword;
    }
    if matches!(
        byte,
        b'"' | b'\'' | b'{' | b'}' | b'(' | b')' | b'[' | b']' | b';' | b',' | b'\\'
    ) {
        return Scope::Punctuation;
    }
    if matches!(byte, b'.' | b'|') {
        return Scope::PunctuationImportant;
    }
    if !node.is_named() || is_operator(kind) {
        return Scope::Keyword;
    }
    Scope::Invalid
}

#[allow(dead_code)]
fn identifier_scope(node: Node<'_>, bytes: &[u8]) -> SynHlColorScope {
    use SynHlColorScope as Scope;
    let Some(parent) = node.parent() else {
        return Scope::Variable;
    };
    let kind = parent.kind();
    if kind.contains("function_declaration") || matches!(kind, "FnProto") {
        return Scope::VariableFunction;
    }
    if kind.contains("parameter") || matches!(kind, "ParamDecl") {
        return Scope::VariableParameter;
    }
    if kind.contains("container_field") || matches!(kind, "ContainerField") {
        return Scope::VariableConstant;
    }
    if kind.contains("variable_declaration") || matches!(kind, "VarDecl") {
        let declaration = &bytes[parent.byte_range()];
        return if declaration.starts_with(b"var") {
            Scope::PunctuationImportant
        } else {
            Scope::VariableConstant
        };
    }
    if kind.contains("call") || matches!(kind, "FieldOrFnCall") {
        return Scope::VariableFunction;
    }
    Scope::Variable
}

fn is_primitive(kind: &str) -> bool {
    matches!(
        kind,
        "void"
            | "bool"
            | "usize"
            | "isize"
            | "type"
            | "anytype"
            | "comptime_int"
            | "comptime_float"
    ) || kind.strip_prefix(['i', 'u', 'f']).is_some_and(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn is_keyword(kind: &str) -> bool {
    matches!(
        kind,
        "align"
            | "allowzero"
            | "and"
            | "anyframe"
            | "anytype"
            | "asm"
            | "async"
            | "await"
            | "break"
            | "catch"
            | "comptime"
            | "continue"
            | "defer"
            | "else"
            | "enum"
            | "errdefer"
            | "error"
            | "export"
            | "extern"
            | "for"
            | "if"
            | "inline"
            | "linksection"
            | "noalias"
            | "noinline"
            | "nosuspend"
            | "opaque"
            | "or"
            | "orelse"
            | "packed"
            | "pub"
            | "resume"
            | "return"
            | "struct"
            | "suspend"
            | "switch"
            | "test"
            | "threadlocal"
            | "try"
            | "union"
            | "unreachable"
            | "usingnamespace"
            | "volatile"
            | "while"
    )
}

fn is_operator(kind: &str) -> bool {
    kind.bytes().all(|byte| {
        matches!(
            byte,
            b'+' | b'-'
                | b'*'
                | b'/'
                | b'%'
                | b'='
                | b'!'
                | b'<'
                | b'>'
                | b'&'
                | b'^'
                | b'?'
                | b':'
        )
    })
}
