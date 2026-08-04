use tree_sitter::Node;

use super::{identifier_end, SynHlColorScope};

pub(super) fn chain_start_offset(kind: &str, source: &[u8]) -> usize {
    if kind.contains("function_declaration") && source.starts_with(b"pub ") {
        4
    } else {
        0
    }
}

pub(super) fn scopes(bytes: &[u8]) -> Vec<SynHlColorScope> {
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

#[allow(dead_code)]
fn node_scope(node: Node<'_>, bytes: &[u8], byte_index: usize) -> SynHlColorScope {
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
