use super::{identifier_end, SynHlColorScope as Scope};

/// Bytes to skip at the start of a node so that expanding a selection over an
/// item does not first swallow its visibility modifier.
pub(super) fn chain_start_offset(kind: &str, source: &[u8]) -> usize {
    if !kind.ends_with("_item") {
        return 0;
    }
    let Some(rest) = source.strip_prefix(b"pub") else {
        return 0;
    };
    let mut offset = "pub".len();
    if rest.first() == Some(&b'(') {
        let Some(close) = rest.iter().position(|byte| *byte == b')') else {
            return 0;
        };
        offset += close + 1;
    }
    let spaces = source[offset..]
        .iter()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count();
    if spaces == 0 {
        0
    } else {
        offset + spaces
    }
}

pub(super) fn scopes(bytes: &[u8]) -> Vec<Scope> {
    let mut scopes = vec![Scope::Invalid; bytes.len()];
    let mut index = 0;
    let mut declaration: Option<Scope> = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() {
            scopes[index] = if index == 0 {
                Scope::Unstyled
            } else {
                scopes[index - 1]
            };
            index += 1;
            continue;
        }
        if bytes[index..].starts_with(b"//") {
            index = line_comment(bytes, &mut scopes, index);
            continue;
        }
        if bytes[index..].starts_with(b"/*") {
            index = block_comment(bytes, &mut scopes, index);
            continue;
        }
        if byte == b'#' {
            if let Some(end) = attribute_introducer(bytes, index) {
                scopes[index..end].fill(Scope::Keyword);
                index = end;
                continue;
            }
        }
        if let Some(literal) = string_start(bytes, index) {
            index = string_literal(bytes, &mut scopes, index, literal);
            continue;
        }
        if byte == b'\'' {
            index = quoted(bytes, &mut scopes, index);
            continue;
        }
        if byte.is_ascii_alphabetic() || byte == b'_' {
            index = word(bytes, &mut scopes, index, &mut declaration);
            continue;
        }
        if byte.is_ascii_digit() {
            index = number(bytes, &mut scopes, index);
            continue;
        }
        scopes[index] = punctuation_scope(byte);
        index += 1;
    }
    scopes
}

fn line_comment(bytes: &[u8], scopes: &mut [Scope], start: usize) -> usize {
    let end = bytes[start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |offset| start + offset);
    let line = &bytes[start..end];
    let doc = line.starts_with(b"///") || line.starts_with(b"//!");
    let marker_len = if doc { 3 } else { 2 };
    let (marker, body) = comment_scopes(doc);
    let split = (start + marker_len).min(end);
    scopes[start..split].fill(marker);
    scopes[split..end].fill(body);
    end
}

fn block_comment(bytes: &[u8], scopes: &mut [Scope], start: usize) -> usize {
    let doc =
        matches!(bytes.get(start + 2), Some(b'*' | b'!')) && bytes.get(start + 3) != Some(&b'/');
    let marker_len = if doc { 3 } else { 2 };
    let (marker, body) = comment_scopes(doc);
    let mut index = (start + marker_len).min(bytes.len());
    scopes[start..index].fill(marker);
    let mut depth = 1usize;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"/*") {
            depth += 1;
            scopes[index..index + 2].fill(marker);
            index += 2;
            continue;
        }
        if bytes[index..].starts_with(b"*/") {
            depth -= 1;
            scopes[index..index + 2].fill(marker);
            index += 2;
            if depth == 0 {
                break;
            }
            continue;
        }
        scopes[index] = body;
        index += 1;
    }
    index
}

const fn comment_scopes(doc: bool) -> (Scope, Scope) {
    if doc {
        (Scope::Keyword, Scope::MarkdownPlainText)
    } else {
        (Scope::Punctuation, Scope::Comment)
    }
}

fn attribute_introducer(bytes: &[u8], start: usize) -> Option<usize> {
    let mut end = start + 1;
    if bytes.get(end) == Some(&b'!') {
        end += 1;
    }
    if bytes.get(end) == Some(&b'[') {
        Some(end + 1)
    } else {
        None
    }
}

/// A string literal opener: the `b`/`c`/`r` prefix letters, any raw string
/// hashes and the opening quote.
struct StringLiteral {
    /// Bytes between the start of the literal and the opening quote.
    prefix: usize,
    hashes: usize,
    raw: bool,
}

fn string_start(bytes: &[u8], start: usize) -> Option<StringLiteral> {
    let mut cursor = start;
    if matches!(bytes.get(cursor), Some(b'b' | b'c')) {
        cursor += 1;
    }
    let raw = bytes.get(cursor) == Some(&b'r');
    let mut hashes = 0;
    if raw {
        cursor += 1;
        while bytes.get(cursor) == Some(&b'#') {
            cursor += 1;
            hashes += 1;
        }
    }
    if bytes.get(cursor) != Some(&b'"') {
        return None;
    }
    Some(StringLiteral {
        prefix: cursor - start,
        hashes,
        raw,
    })
}

fn string_literal(
    bytes: &[u8],
    scopes: &mut [Scope],
    start: usize,
    literal: StringLiteral,
) -> usize {
    let quote = start + literal.prefix;
    scopes[start..quote - literal.hashes].fill(Scope::KeywordStorage);
    scopes[quote - literal.hashes..=quote].fill(Scope::Punctuation);
    let mut index = quote + 1;
    if literal.raw {
        while index < bytes.len() {
            if bytes[index] == b'"'
                && bytes
                    .get(index + 1..index + 1 + literal.hashes)
                    .is_some_and(|tail| tail.iter().all(|byte| *byte == b'#'))
            {
                let end = index + 1 + literal.hashes;
                scopes[index..end].fill(Scope::Punctuation);
                return end;
            }
            scopes[index] = Scope::LiteralString;
            index += 1;
        }
        return index;
    }
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                scopes[index] = Scope::Punctuation;
                return index + 1;
            }
            b'\\' => index = escape(bytes, scopes, index),
            b'{' | b'}' => index = format_placeholder(bytes, scopes, index),
            _ => {
                scopes[index] = Scope::LiteralString;
                index += 1;
            }
        }
    }
    index
}

/// Colours a `\`-escape: the backslash is punctuation, and the escaped value is
/// either the string colour (when it stands for the quoted character itself) or
/// a literal colour (when it stands for something else).
fn escape(bytes: &[u8], scopes: &mut [Scope], start: usize) -> usize {
    scopes[start] = Scope::Punctuation;
    let mut index = start + 1;
    let Some(escape) = bytes.get(index).copied() else {
        return index;
    };
    scopes[index] = match escape {
        b'n' | b'r' | b't' | b'0' => Scope::Literal,
        b'\\' | b'\'' | b'"' => Scope::LiteralString,
        b'x' | b'u' => Scope::KeywordStorage,
        byte if byte.is_ascii_whitespace() => Scope::Punctuation,
        _ => Scope::Invalid,
    };
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
    index
}

fn format_placeholder(bytes: &[u8], scopes: &mut [Scope], start: usize) -> usize {
    let doubled = bytes.get(start + 1) == Some(&bytes[start]);
    if bytes[start] == b'{' {
        scopes[start] = Scope::Punctuation;
        if doubled {
            scopes[start + 1] = Scope::LiteralString;
        }
    } else if doubled {
        scopes[start] = Scope::LiteralString;
        scopes[start + 1] = Scope::Punctuation;
    } else {
        scopes[start] = Scope::Punctuation;
    }
    if doubled {
        start + 2
    } else {
        start + 1
    }
}

fn quoted(bytes: &[u8], scopes: &mut [Scope], start: usize) -> usize {
    if !is_character_literal(bytes, start) {
        let end = identifier_end(bytes, start + 1);
        scopes[start..end].fill(Scope::Keyword);
        return end;
    }
    scopes[start] = Scope::Punctuation;
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' => {
                scopes[index] = Scope::Punctuation;
                return index + 1;
            }
            b'\\' => index = escape(bytes, scopes, index),
            _ => {
                scopes[index] = Scope::LiteralString;
                index += 1;
            }
        }
    }
    index
}

fn is_character_literal(bytes: &[u8], start: usize) -> bool {
    match bytes.get(start + 1) {
        Some(b'\\') => true,
        Some(byte) if byte.is_ascii_alphanumeric() || *byte == b'_' => {
            bytes.get(start + 2) == Some(&b'\'')
        }
        Some(_) => true,
        None => false,
    }
}

fn word(
    bytes: &[u8],
    scopes: &mut [Scope],
    start: usize,
    declaration: &mut Option<Scope>,
) -> usize {
    let mut end = identifier_end(bytes, start);
    let token = std::str::from_utf8(&bytes[start..end]).unwrap_or_default();
    if token == "b" && bytes.get(end) == Some(&b'\'') {
        scopes[start..end].fill(Scope::KeywordStorage);
        return end;
    }
    if token == "r" && bytes.get(end) == Some(&b'#') {
        end = identifier_end(bytes, end + 1);
        scopes[start..end].fill(Scope::Variable);
        return end;
    }
    let scope = if let Some(declared) = declared_scope(token) {
        *declaration = Some(declared);
        Scope::KeywordStorage
    } else if token == "mut" {
        if *declaration == Some(Scope::Variable) {
            *declaration = Some(Scope::VariableMutable);
        }
        Scope::KeywordStorage
    } else if let Some(declared) = declaration.take() {
        declared
    } else if matches!(token, "true" | "false") {
        Scope::Literal
    } else if is_primitive(token) {
        Scope::KeywordPrimitiveType
    } else if is_keyword(token) {
        Scope::Keyword
    } else if bytes.get(end) == Some(&b'!') && bytes.get(end + 1) != Some(&b'=') {
        end += 1;
        Scope::VariableFunction
    } else if bytes.get(end) == Some(&b'(') {
        Scope::VariableFunction
    } else if is_constant_name(token) {
        Scope::VariableConstant
    } else if is_type_name(token) {
        Scope::KeywordPrimitiveType
    } else {
        Scope::Variable
    };
    scopes[start..end].fill(scope);
    end
}

fn number(bytes: &[u8], scopes: &mut [Scope], start: usize) -> usize {
    let mut index = start + 1;
    while index < bytes.len() {
        let byte = bytes[index];
        let digit = byte.is_ascii_hexdigit() || matches!(byte, b'x' | b'o' | b'b' | b'_');
        let fraction = byte == b'.' && bytes.get(index + 1).is_some_and(u8::is_ascii_digit);
        let exponent = matches!(byte, b'+' | b'-') && matches!(bytes[index - 1], b'e' | b'E');
        if !(digit || fraction || exponent) {
            break;
        }
        index += 1;
    }
    scopes[start..index].fill(Scope::Literal);
    if index - start >= 2 && bytes[start] == b'0' && matches!(bytes[start + 1], b'x' | b'o' | b'b')
    {
        scopes[start..start + 2].fill(Scope::KeywordStorage);
    }
    index
}

fn punctuation_scope(byte: u8) -> Scope {
    if matches!(
        byte,
        b'{' | b'}' | b'(' | b')' | b'[' | b']' | b';' | b',' | b'\\' | b'#'
    ) {
        Scope::Punctuation
    } else if matches!(byte, b'.' | b'|' | b':') {
        Scope::PunctuationImportant
    } else {
        Scope::Keyword
    }
}

/// The colour given to the name introduced by a declaration keyword.
fn declared_scope(token: &str) -> Option<Scope> {
    match token {
        "let" => Some(Scope::Variable),
        "const" | "static" => Some(Scope::VariableConstant),
        "fn" => Some(Scope::VariableFunction),
        _ => None,
    }
}

fn is_primitive(token: &str) -> bool {
    matches!(token, "bool" | "char" | "str" | "usize" | "isize")
        || token.strip_prefix(['i', 'u', 'f']).is_some_and(|digits| {
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
}

/// `SCREAMING_SNAKE_CASE`. Single letters are excluded so that generic type
/// parameters such as `T` keep the type colour.
fn is_constant_name(token: &str) -> bool {
    token.len() > 1
        && token.bytes().any(|byte| byte.is_ascii_uppercase())
        && token
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn is_type_name(token: &str) -> bool {
    token.starts_with(|character: char| character.is_ascii_uppercase())
}

fn is_keyword(token: &str) -> bool {
    matches!(
        token,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "macro_rules"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "type"
            | "union"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "yield"
    )
}
