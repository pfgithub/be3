use block_client::{blocks::text::TextDocument, BlockHandle};
use tree_sitter::{InputEdit, Node, Parser, Point, Tree};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
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
            Self::Unstyled => "unstyled",
            Self::Invisible => "invisible",
        }
    }
}

pub struct SyntaxHighlight {
    scopes: Vec<SynHlColorScope>,
}

impl SyntaxHighlight {
    pub(crate) fn plaintext(len: usize) -> Self {
        Self {
            scopes: vec![SynHlColorScope::Unstyled; len],
        }
    }

    pub fn advance_and_read(&self, byte_index: usize) -> SynHlColorScope {
        self.scopes
            .get(byte_index)
            .copied()
            .unwrap_or(SynHlColorScope::Invalid)
    }
}

pub struct Highlighter {
    document: BlockHandle<TextDocument>,
    parser: Parser,
    tree: Option<Tree>,
    parsed_revision: Option<u64>,
    parsed_bytes: Vec<u8>,
    language: Language,
}

impl Highlighter {
    pub fn new(document: BlockHandle<TextDocument>, language: Language) -> Self {
        let mut parser = Parser::new();
        match language {
            Language::Zig => parser
                .set_language(&tree_sitter_zig::LANGUAGE.into())
                .expect("tree-sitter Zig language is incompatible"),
        }
        Self {
            document,
            parser,
            tree: None,
            parsed_revision: None,
            parsed_bytes: Vec::new(),
            language,
        }
    }

    fn ensure_parsed(&mut self) -> Option<()> {
        let revision = self.document.revision();
        if self.parsed_revision == Some(revision) && self.tree.is_some() {
            return Some(());
        }
        let document = self.document.read()?;
        let bytes = document.bytes();
        if let Some(tree) = self.tree.as_mut() {
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
            tree.edit(&InputEdit {
                start_byte: prefix,
                old_end_byte: old_end,
                new_end_byte: new_end,
                start_position: byte_point(&self.parsed_bytes, prefix),
                old_end_position: byte_point(&self.parsed_bytes, old_end),
                new_end_position: byte_point(bytes, new_end),
            });
        }
        self.tree = self.parser.parse(bytes, self.tree.as_ref());
        self.parsed_bytes.clear();
        self.parsed_bytes.extend_from_slice(bytes);
        self.parsed_revision = Some(revision);
        Some(())
    }

    pub fn highlight(&mut self) -> SyntaxHighlight {
        if self.ensure_parsed().is_none() {
            return SyntaxHighlight { scopes: Vec::new() };
        }
        let document = self.document.read().expect("parsed document disappeared");
        let bytes = document.bytes();
        let mut scopes = match self.language {
            Language::Zig => zig_scopes(bytes),
        };
        for index in 1..scopes.len() {
            if bytes[index].is_ascii_whitespace() {
                scopes[index] = scopes[index - 1];
            }
        }
        SyntaxHighlight { scopes }
    }

    pub(crate) fn node_chain(&mut self, start: usize, end: usize) -> Vec<(usize, usize)> {
        if self.ensure_parsed().is_none() {
            return Vec::new();
        }
        let tree = self.tree.as_ref().expect("parser omitted a syntax tree");
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
