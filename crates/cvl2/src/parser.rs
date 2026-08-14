use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::compiler::{throw_err, Env, PositionedError};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenPosition {
    pub fyl: String,
    pub idx: usize,
    pub lyn: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpTag {
    Sep,
    Def,
    Pub,
    Var,
    Assign,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BracketTag {
    Map,
    List,
    Code,
    ColonCall,
    ArrowFn,
    String,
    InlineComment,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawTag {
    Return,
    Discard,
    Void,
    String,
    Comment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierTag {
    Normal,
    Access,
    Builtin,
    Number,
    Discard,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IdentifierToken {
    pub pos: TokenPosition,
    pub str: String,
    pub ident_tag: IdentifierTag,
    pub ident_tag_raw: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhitespaceToken {
    pub pos: TokenPosition,
    pub nl: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperatorToken {
    pub pos: TokenPosition,
    pub op: String,
    pub op_tag: OpTag,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperatorSegmentToken {
    pub pos: TokenPosition,
    pub items: Vec<SyntaxNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockToken {
    pub pos: TokenPosition,
    pub start: String,
    pub end: String,
    pub items: Vec<SyntaxNode>,
    pub tag: BracketTag,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinaryExpressionToken {
    pub pos: TokenPosition,
    pub prec: usize,
    pub tag: OpTag,
    pub items: Vec<SyntaxNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawToken {
    pub pos: TokenPosition,
    pub raw: String,
    pub tag: RawTag,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ErrToken {
    pub pos: TokenPosition,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyntaxNode {
    Identifier(IdentifierToken),
    Whitespace(WhitespaceToken),
    Operator(OperatorToken),
    OperatorSegment(OperatorSegmentToken),
    Block(Box<BlockToken>),
    BinaryExpression(Box<BinaryExpressionToken>),
    Raw(RawToken),
    Err(ErrToken),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorStyle {
    Note,
    Error,
    Todo,
    Unreachable,
    Warning,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TokenizationErrorEntry {
    pub pos: Option<TokenPosition>,
    pub style: ErrorStyle,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceEntry {
    pub pos: TokenPosition,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TokenizationError {
    pub entries: Vec<TokenizationErrorEntry>,
    pub trace: Vec<TraceEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TokenizationResult {
    pub result: Vec<SyntaxNode>,
    pub errors: Vec<TokenizationError>,
}

pub struct Source {
    pub text: String,
    chars: Vec<char>,
    pub current_index: usize,
    pub current_line: usize,
    pub current_col: usize,
    pub filename: String,
    pub current_line_indent_level: usize,
}

fn calculate_indent(chars: &[char], current_index: usize) -> usize {
    chars[current_index.min(chars.len())..]
        .iter()
        .take_while(|&&c| c == ' ')
        .count()
}

impl Source {
    pub fn new(filename: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let chars: Vec<char> = text.chars().collect();
        let current_line_indent_level = calculate_indent(&chars, 0);
        Source {
            text,
            chars,
            current_index: 0,
            current_line: 1,
            current_col: 1,
            filename: filename.into(),
            current_line_indent_level,
        }
    }

    pub fn peek(&self) -> Option<char> {
        self.chars.get(self.current_index).copied()
    }

    pub fn take(&mut self) -> Option<char> {
        let character = self.peek();
        if let Some(c) = character {
            self.current_index += 1;
            if c == '\n' {
                self.current_line += 1;
                self.current_col = 1;
                self.current_line_indent_level = calculate_indent(&self.chars, self.current_index);
            } else {
                self.current_col += 1;
            }
        }
        character
    }

    pub fn revert(&mut self, pos: &TokenPosition) {
        self.filename = pos.fyl.clone();
        self.current_index = pos.idx;
        self.current_line = pos.lyn;
        self.current_col = pos.col;
    }

    pub fn get_position(&self) -> TokenPosition {
        TokenPosition {
            fyl: self.filename.clone(),
            idx: self.current_index,
            lyn: self.current_line,
            col: self.current_col,
        }
    }

    fn text_slice(&self, start: usize, end: usize) -> String {
        self.chars[start..end].iter().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenizerMode {
    Regular,
    InString,
    InlineComment,
}

const IN_STRING_QUOTE: &str = "<in_string>\"";
const IN_INLINE_COMMENT_END: &str = "<in_inline_comment>end";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigStyle {
    Open,
    Close,
    Join,
}

#[derive(Debug, Clone, Copy)]
struct ConfigEntry {
    style: ConfigStyle,
    prec: usize,
    close: Option<&'static str>,
    auto_open: bool,
    op_tag: Option<OpTag>,
    bracket_tag: Option<BracketTag>,
}

// Mirrors the TS `mkconfig` table: entries are grouped, and every entry in a
// group shares the group's precedence (the group's position in this list).
fn lookup_config(token: &str) -> Option<ConfigEntry> {
    let (style, prec, close, bracket_tag, op_tag) = match token {
        "(" => (
            ConfigStyle::Open,
            0,
            Some(")"),
            Some(BracketTag::List),
            None,
        ),
        "{" => (
            ConfigStyle::Open,
            0,
            Some("}"),
            Some(BracketTag::Code),
            None,
        ),
        "[" => (ConfigStyle::Open, 0, Some("]"), Some(BracketTag::Map), None),
        ")" => (ConfigStyle::Close, 0, None, Some(BracketTag::List), None),
        "}" => (ConfigStyle::Close, 0, None, Some(BracketTag::Code), None),
        "]" => (ConfigStyle::Close, 0, None, Some(BracketTag::Map), None),
        "\\(" => (
            ConfigStyle::Open,
            0,
            Some(")"),
            Some(BracketTag::List),
            None,
        ),

        "," => (ConfigStyle::Join, 1, None, None, Some(OpTag::Sep)),
        ";" => (ConfigStyle::Join, 1, None, None, Some(OpTag::Sep)),
        "\n" => (ConfigStyle::Join, 1, None, None, Some(OpTag::Sep)),
        "\n\n" => (ConfigStyle::Join, 1, None, None, Some(OpTag::Sep)),

        "::" => (ConfigStyle::Join, 2, None, None, Some(OpTag::Def)),
        ".=" => (ConfigStyle::Join, 2, None, None, Some(OpTag::Pub)),
        ":=" => (ConfigStyle::Join, 2, None, None, Some(OpTag::Var)),

        ":" => (
            ConfigStyle::Open,
            3,
            None,
            Some(BracketTag::ColonCall),
            None,
        ),
        "=>" => (ConfigStyle::Open, 3, None, Some(BracketTag::ArrowFn), None),

        "=" => (ConfigStyle::Join, 4, None, None, Some(OpTag::Assign)),

        "\"" => (
            ConfigStyle::Open,
            5,
            Some(IN_STRING_QUOTE),
            Some(BracketTag::String),
            None,
        ),
        s if s == IN_STRING_QUOTE => (ConfigStyle::Close, 5, None, Some(BracketTag::String), None),
        "//" => (
            ConfigStyle::Open,
            5,
            Some(IN_INLINE_COMMENT_END),
            Some(BracketTag::InlineComment),
            None,
        ),
        s if s == IN_INLINE_COMMENT_END => (
            ConfigStyle::Close,
            5,
            None,
            Some(BracketTag::InlineComment),
            None,
        ),

        _ => return None,
    };
    Some(ConfigEntry {
        style,
        prec,
        close,
        auto_open: false,
        op_tag,
        bracket_tag,
    })
}

fn set_mode_for_bracket_tag(tag: BracketTag) -> Option<TokenizerMode> {
    match tag {
        BracketTag::String => Some(TokenizerMode::InString),
        BracketTag::InlineComment => Some(TokenizerMode::InlineComment),
        _ => None,
    }
}

fn raw_tag_for(token: &str) -> Option<RawTag> {
    match token {
        "->" => Some(RawTag::Return),
        "_" => Some(RawTag::Discard),
        _ => None,
    }
}

fn ident_tag_for(c: char) -> Option<IdentifierTag> {
    match c {
        '.' => Some(IdentifierTag::Access),
        '#' => Some(IdentifierTag::Builtin),
        _ => None,
    }
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_ws_char(c: char) -> bool {
    c.is_whitespace()
}

const OPERATOR_CHARS: &str = "~!@$%^&*-=+|/<>:.";

type NodeList = Rc<RefCell<Vec<BuilderNode>>>;

fn new_node_list() -> NodeList {
    Rc::new(RefCell::new(Vec::new()))
}

// Mutable-during-parsing counterpart of `SyntaxNode`: container fields are
// `Rc<RefCell<Vec<_>>>` so a stack entry's `val` can alias the `items` of the
// tree node it was created from, matching the TS tokenizer's array-reference
// aliasing. `freeze_list` converts this into the plain `SyntaxNode` tree once
// tokenization is done.
enum BuilderNode {
    Identifier(IdentifierToken),
    Whitespace(WhitespaceToken),
    Operator(OperatorToken),
    OperatorSegment {
        pos: TokenPosition,
        items: NodeList,
    },
    Block {
        pos: TokenPosition,
        start: String,
        end: String,
        items: NodeList,
        tag: BracketTag,
    },
    BinaryExpression {
        pos: TokenPosition,
        prec: usize,
        tag: OpTag,
        items: NodeList,
    },
    Raw(RawToken),
}

fn freeze_list(list: &NodeList) -> Vec<SyntaxNode> {
    list.borrow_mut().drain(..).map(freeze_node).collect()
}

fn freeze_node(node: BuilderNode) -> SyntaxNode {
    match node {
        BuilderNode::Identifier(t) => SyntaxNode::Identifier(t),
        BuilderNode::Whitespace(t) => SyntaxNode::Whitespace(t),
        BuilderNode::Operator(t) => SyntaxNode::Operator(t),
        BuilderNode::OperatorSegment { pos, items } => {
            SyntaxNode::OperatorSegment(OperatorSegmentToken {
                pos,
                items: freeze_list(&items),
            })
        }
        BuilderNode::Block {
            pos,
            start,
            end,
            items,
            tag,
        } => SyntaxNode::Block(Box::new(BlockToken {
            pos,
            start,
            end,
            items: freeze_list(&items),
            tag,
        })),
        BuilderNode::BinaryExpression {
            pos,
            prec,
            tag,
            items,
        } => SyntaxNode::BinaryExpression(Box::new(BinaryExpressionToken {
            pos,
            prec,
            tag,
            items: freeze_list(&items),
        })),
        BuilderNode::Raw(t) => SyntaxNode::Raw(t),
    }
}

struct TokenizerStackItem {
    pos: TokenPosition,
    char: String,
    indent: i64,
    val: NodeList,
    op_sup_val: Option<NodeList>,
    prec: usize,
    auto_close: bool,
    tag: Option<OpTag>,
    bracket_tag: Option<BracketTag>,
}

pub fn tokenize(source: &mut Source) -> TokenizationResult {
    let mut current_syntax_nodes = new_node_list();
    let mut errors: Vec<TokenizationError> = Vec::new();
    let mut parse_stack: Vec<TokenizerStackItem> = Vec::new();

    parse_stack.push(TokenizerStackItem {
        pos: source.get_position(),
        char: String::new(),
        indent: -1,
        val: Rc::clone(&current_syntax_nodes),
        op_sup_val: None,
        prec: 0,
        auto_close: false,
        tag: None,
        bracket_tag: None,
    });

    while source.peek().is_some() {
        let start = source.get_position();

        let mode = parse_stack
            .last()
            .and_then(|item| item.bracket_tag)
            .and_then(set_mode_for_bracket_tag)
            .unwrap_or(TokenizerMode::Regular);

        let current_token: String;
        match mode {
            TokenizerMode::Regular => {
                let first_char = source
                    .take()
                    .expect("peek() confirmed a character is available");

                if is_ident_char(first_char) {
                    let mut count = 1usize;
                    while source.peek().is_some_and(is_ident_char) {
                        source.take();
                        count += 1;
                    }
                    let str_value = source.text_slice(start.idx, source.current_index);
                    let ident_tag = if first_char.is_ascii_digit() {
                        IdentifierTag::Number
                    } else if first_char == '_' && count == 1 {
                        IdentifierTag::Discard
                    } else {
                        IdentifierTag::Normal
                    };
                    current_syntax_nodes
                        .borrow_mut()
                        .push(BuilderNode::Identifier(IdentifierToken {
                            pos: start,
                            str: str_value,
                            ident_tag,
                            ident_tag_raw: String::new(),
                        }));
                    continue;
                }

                if let Some(tag) = ident_tag_for(first_char) {
                    let before_attempt = source.get_position();

                    while source.peek().is_some_and(is_ident_char) {
                        source.take();
                    }
                    let len = source.current_index - start.idx - 1;
                    if len > 0 {
                        current_syntax_nodes
                            .borrow_mut()
                            .push(BuilderNode::Identifier(IdentifierToken {
                                pos: start.clone(),
                                str: source.text_slice(start.idx + 1, source.current_index),
                                ident_tag: tag,
                                ident_tag_raw: first_char.to_string(),
                            }));
                        continue;
                    } else {
                        source.revert(&before_attempt);
                    }
                }

                if is_ws_char(first_char) {
                    while source.peek().is_some_and(is_ws_char) {
                        source.take();
                    }
                    let consumed = source.text_slice(start.idx, source.current_index);
                    let nl_count = consumed.matches('\n').count();
                    current_token = if nl_count > 1 {
                        "\n\n".to_string()
                    } else if nl_count == 1 {
                        "\n".to_string()
                    } else {
                        " ".to_string()
                    };
                } else if "()[]{},;\"'`".contains(first_char) {
                    current_token = first_char.to_string();
                } else if OPERATOR_CHARS.contains(first_char) {
                    while source.peek().is_some_and(|c| OPERATOR_CHARS.contains(c)) {
                        source.take();
                    }
                    current_token = source.text_slice(start.idx, source.current_index);
                } else if first_char == '\\' {
                    current_token = "\\".to_string();
                } else {
                    current_token = first_char.to_string();
                }
            }
            TokenizerMode::InString => {
                let (request, token_value) = loop {
                    let peek = source.peek();
                    if peek == Some('\\') {
                        let revert = source.get_position();
                        source.take();
                        let esc_first = source.peek();
                        if esc_first == Some('"') || esc_first == Some('\\') {
                            source.take();
                        } else if esc_first == Some('(') {
                            source.take();
                            let request = source.get_position();
                            source.revert(&revert);
                            break (request, "\\(".to_string());
                        }
                    } else if peek == Some('"') {
                        let revert = source.get_position();
                        source.take();
                        let request = source.get_position();
                        source.revert(&revert);
                        break (request, IN_STRING_QUOTE.to_string());
                    } else {
                        source.take();
                    }
                };
                let raw_value = source.text_slice(start.idx, source.current_index);
                current_syntax_nodes
                    .borrow_mut()
                    .push(BuilderNode::Raw(RawToken {
                        pos: start.clone(),
                        raw: raw_value,
                        tag: RawTag::String,
                    }));
                source.revert(&request);
                current_token = token_value;
            }
            TokenizerMode::InlineComment => {
                let request = loop {
                    let peek = source.peek();
                    if peek == Some('\n') {
                        break source.get_position();
                    } else {
                        source.take();
                    }
                };
                let raw_value = source.text_slice(start.idx, source.current_index);
                current_syntax_nodes
                    .borrow_mut()
                    .push(BuilderNode::Raw(RawToken {
                        pos: start.clone(),
                        raw: raw_value,
                        tag: RawTag::Comment,
                    }));
                source.revert(&request);
                current_token = IN_INLINE_COMMENT_END.to_string();
            }
        }

        let cfg = lookup_config(&current_token);

        match cfg.map(|c| c.style) {
            Some(ConfigStyle::Open) => {
                let cfg = cfg.expect("style implies cfg is Some");
                let new_block_items = new_node_list();
                current_syntax_nodes.borrow_mut().push(BuilderNode::Block {
                    pos: start.clone(),
                    start: current_token.clone(),
                    end: cfg.close.unwrap_or("").to_string(),
                    items: Rc::clone(&new_block_items),
                    tag: cfg.bracket_tag.unwrap_or(BracketTag::None),
                });
                parse_stack.push(TokenizerStackItem {
                    pos: start.clone(),
                    char: cfg.close.unwrap_or("").to_string(),
                    indent: source.current_line_indent_level as i64,
                    val: Rc::clone(&new_block_items),
                    op_sup_val: None,
                    prec: cfg.prec,
                    auto_close: cfg.close.is_none(),
                    tag: None,
                    bracket_tag: Some(cfg.bracket_tag.unwrap_or(BracketTag::None)),
                });
                current_syntax_nodes = new_block_items;
            }
            Some(ConfigStyle::Close) => {
                let cfg = cfg.expect("style implies cfg is Some");
                let current_indent = source.current_line_indent_level as i64;

                loop {
                    if parse_stack.is_empty() {
                        break;
                    }
                    let last_stack_item = parse_stack.pop().expect("checked non-empty above");

                    if last_stack_item.char == current_token
                        && last_stack_item.indent == current_indent
                    {
                        current_syntax_nodes =
                            Rc::clone(&parse_stack.last().expect("root stays on stack").val);
                        break;
                    }

                    if last_stack_item.indent < current_indent || last_stack_item.prec < cfg.prec {
                        parse_stack.push(last_stack_item);
                        if cfg.auto_open {
                            let val = Rc::clone(&parse_stack.last().expect("just pushed").val);
                            let first_non_ws = {
                                let borrowed = val.borrow();
                                let mut i = 0;
                                while i < borrowed.len() {
                                    if !matches!(borrowed[i], BuilderNode::Whitespace(_)) {
                                        break;
                                    }
                                    i += 1;
                                }
                                i
                            };
                            let prev_items = val.borrow_mut().split_off(first_non_ws);
                            val.borrow_mut().push(BuilderNode::Block {
                                pos: start.clone(),
                                start: String::new(),
                                end: current_token.clone(),
                                items: Rc::new(RefCell::new(prev_items)),
                                tag: cfg.bracket_tag.unwrap_or(BracketTag::None),
                            });
                        } else {
                            errors.push(TokenizationError {
                                entries: vec![TokenizationErrorEntry {
                                    message: "extra close bracket".to_string(),
                                    style: ErrorStyle::Error,
                                    pos: Some(start.clone()),
                                }],
                                trace: Vec::new(),
                            });
                        }
                        break;
                    } else {
                        if !last_stack_item.auto_close {
                            errors.push(TokenizationError {
                                entries: vec![
                                    TokenizationErrorEntry {
                                        message: "open bracket missing close bracket".to_string(),
                                        style: ErrorStyle::Error,
                                        pos: Some(last_stack_item.pos.clone()),
                                    },
                                    TokenizationErrorEntry {
                                        message: format!(
                                            "expected {:?} indent '{}', got {:?} indent '{}'",
                                            last_stack_item.char,
                                            last_stack_item.indent,
                                            current_token,
                                            current_indent
                                        ),
                                        style: ErrorStyle::Note,
                                        pos: Some(start.clone()),
                                    },
                                ],
                                trace: Vec::new(),
                            });
                        }
                        current_syntax_nodes =
                            Rc::clone(&parse_stack.last().expect("root stays on stack").val);
                    }
                }
                assert!(!parse_stack.is_empty(), "ups parsestack len 0!");
            }
            Some(ConfigStyle::Join) => {
                let cfg = cfg.expect("style implies cfg is Some");
                let operator_precedence = cfg.prec;
                let mut target_index: Option<usize> = None;

                loop {
                    if parse_stack.is_empty() {
                        break;
                    }
                    let last_index = parse_stack.len() - 1;
                    let last_prec = parse_stack[last_index].prec;

                    if last_prec == operator_precedence {
                        if parse_stack[last_index].tag != cfg.op_tag {
                            errors.push(TokenizationError {
                                entries: vec![
                                    TokenizationErrorEntry {
                                        message: "mixing operators disallowed".to_string(),
                                        style: ErrorStyle::Error,
                                        pos: Some(start.clone()),
                                    },
                                    TokenizationErrorEntry {
                                        message: "previous operator here".to_string(),
                                        style: ErrorStyle::Note,
                                        pos: None,
                                    },
                                ],
                                trace: Vec::new(),
                            });
                        }
                        target_index = Some(last_index);
                        break;
                    } else if last_prec < operator_precedence {
                        let indent = parse_stack[last_index].indent;
                        let last_val = Rc::clone(&parse_stack[last_index].val);
                        let old_contents: Vec<BuilderNode> =
                            last_val.borrow_mut().drain(..).collect();
                        let val_list: NodeList = Rc::new(RefCell::new(old_contents));
                        let op_sup_val: NodeList =
                            Rc::new(RefCell::new(vec![BuilderNode::OperatorSegment {
                                pos: start.clone(),
                                items: Rc::clone(&val_list),
                            }]));
                        last_val.borrow_mut().push(BuilderNode::BinaryExpression {
                            pos: start.clone(),
                            prec: operator_precedence,
                            tag: cfg.op_tag.unwrap_or(OpTag::None),
                            items: Rc::clone(&op_sup_val),
                        });

                        parse_stack.push(TokenizerStackItem {
                            pos: start.clone(),
                            char: current_token.clone(),
                            indent,
                            val: val_list,
                            op_sup_val: Some(op_sup_val),
                            prec: operator_precedence,
                            auto_close: true,
                            tag: Some(cfg.op_tag.unwrap_or(OpTag::None)),
                            bracket_tag: None,
                        });
                        target_index = Some(parse_stack.len() - 1);
                        break;
                    } else {
                        if !parse_stack[last_index].auto_close {
                            errors.push(TokenizationError {
                                entries: vec![
                                    TokenizationErrorEntry {
                                        message: "item is never closed.".to_string(),
                                        style: ErrorStyle::Error,
                                        pos: Some(parse_stack[last_index].pos.clone()),
                                    },
                                    TokenizationErrorEntry {
                                        message: "automatically closed here.".to_string(),
                                        style: ErrorStyle::Note,
                                        pos: Some(start.clone()),
                                    },
                                ],
                                trace: Vec::new(),
                            });
                        }
                        parse_stack.pop();
                    }
                }

                let target_index =
                    target_index.expect("Unreachable: No target block found for comma/semicolon.");
                let next_val = new_node_list();
                {
                    let op_sup_val = parse_stack[target_index]
                        .op_sup_val
                        .clone()
                        .expect("Target block missing opSupVal? Is this reachable?");
                    op_sup_val
                        .borrow_mut()
                        .push(BuilderNode::Operator(OperatorToken {
                            pos: start.clone(),
                            op: current_token.clone(),
                            op_tag: parse_stack[target_index].tag.unwrap_or(OpTag::None),
                        }));
                    op_sup_val.borrow_mut().push(BuilderNode::OperatorSegment {
                        pos: start.clone(),
                        items: Rc::clone(&next_val),
                    });
                    parse_stack[target_index].val = Rc::clone(&next_val);
                }
                current_syntax_nodes = next_val;
            }
            None => {
                if current_token == " " {
                    current_syntax_nodes
                        .borrow_mut()
                        .push(BuilderNode::Whitespace(WhitespaceToken {
                            pos: start.clone(),
                            nl: false,
                        }));
                } else if let Some(raw_tag) = raw_tag_for(&current_token) {
                    current_syntax_nodes
                        .borrow_mut()
                        .push(BuilderNode::Raw(RawToken {
                            pos: start.clone(),
                            raw: current_token.clone(),
                            tag: raw_tag,
                        }));
                } else {
                    errors.push(TokenizationError {
                        entries: vec![TokenizationErrorEntry {
                            message: format!("bad token {current_token:?}"),
                            style: ErrorStyle::Error,
                            pos: Some(start.clone()),
                        }],
                        trace: Vec::new(),
                    });
                }
            }
        }
    }

    TokenizationResult {
        result: freeze_list(&parse_stack[0].val),
        errors,
    }
}

fn pos_add_col(pos: &TokenPosition, col: usize) -> TokenPosition {
    TokenPosition {
        fyl: pos.fyl.clone(),
        lyn: pos.lyn,
        col: pos.col + col,
        idx: pos.col + col,
    }
}

fn unescape_def(c: char) -> Option<char> {
    match c {
        'n' => Some('\n'),
        'r' => Some('\r'),
        '"' => Some('"'),
        '\'' => Some('\''),
        _ => None,
    }
}

fn parse_hex_codepoint(
    env: &Env,
    inner: &str,
    segment_pos: &TokenPosition,
) -> Result<String, PositionedError> {
    if let Some(bad) = inner.find(|c: char| !c.is_ascii_hexdigit()) {
        return Err(throw_err(
            env,
            Some(pos_add_col(segment_pos, bad)),
            "bad character in curly",
            None,
            None,
        ));
    }
    let parsed = u32::from_str_radix(inner, 16).unwrap_or(0);
    if parsed > 0x10FFFF {
        return Err(throw_err(
            env,
            Some(segment_pos.clone()),
            "number out of range",
            None,
            None,
        ));
    }
    match char::from_u32(parsed) {
        Some(ch) => Ok(ch.to_string()),
        None => Err(throw_err(
            env,
            Some(segment_pos.clone()),
            "number out of range",
            None,
            None,
        )),
    }
}

pub fn unescape_string(
    env: &Env,
    segment: &str,
    segment_pos: TokenPosition,
) -> Result<String, PositionedError> {
    if !segment.contains('\\') {
        return Ok(segment.to_string());
    }
    if let Some(newline_index) = segment.find('\n') {
        return Err(throw_err(
            env,
            Some(pos_add_col(&segment_pos, newline_index)),
            "newline is not allowed in escaped string",
            None,
            Some(ErrorStyle::Unreachable),
        ));
    }

    let chars: Vec<char> = segment.chars().collect();
    let mut result = String::new();
    let mut idx = 0usize;
    loop {
        let next_escape = chars[idx..]
            .iter()
            .position(|&c| c == '\\')
            .map(|p| idx + p)
            .unwrap_or(chars.len());
        result.extend(&chars[idx..next_escape]);
        idx = next_escape;
        if chars.get(idx) != Some(&'\\') {
            break;
        }
        idx += 1;
        let escaped = chars.get(idx).copied();
        if let Some(def) = escaped.and_then(unescape_def) {
            idx += 1;
            result.push(def);
        } else if escaped == Some('x') {
            idx += 1;
            let inner: String = chars
                .get(idx..(idx + 2).min(chars.len()))
                .unwrap_or(&[])
                .iter()
                .collect();
            result.push_str(&parse_hex_codepoint(
                env,
                &inner,
                &pos_add_col(&segment_pos, idx),
            )?);
            idx += 2;
        } else if escaped == Some('u') {
            idx += 1;
            let ustart = idx;
            if chars.get(idx) != Some(&'{') {
                return Err(throw_err(
                    env,
                    Some(pos_add_col(&segment_pos, idx)),
                    "expected \\u{ABCD}",
                    None,
                    None,
                ));
            }
            idx += 1;
            let istart = idx;
            let iend = chars[istart..]
                .iter()
                .position(|&c| c == '}')
                .map(|p| istart + p);
            let Some(iend) = iend else {
                return Err(throw_err(
                    env,
                    Some(pos_add_col(&segment_pos, ustart)),
                    "missing close curly",
                    Some(vec![(
                        Some(pos_add_col(&segment_pos, chars.len())),
                        "string ended here".to_string(),
                    )]),
                    None,
                ));
            };
            // Mirrors upstream: this only advances past `{`, not past the
            // whole `}`-terminated escape, so the trailing hex digits and
            // `}` fall through and get appended again as plain text below.
            idx += 1;
            let inner: String = chars[istart..iend].iter().collect();
            result.push_str(&parse_hex_codepoint(
                env,
                &inner,
                &pos_add_col(&segment_pos, istart),
            )?);
        } else {
            let shown = escaped
                .map(|c| c.to_string())
                .unwrap_or_else(|| "<eof>".to_string());
            return Err(throw_err(
                env,
                Some(pos_add_col(&segment_pos, idx)),
                format!("unexpected escape in string: {shown:?}"),
                None,
                None,
            ));
        }
    }
    Ok(result)
}

struct RenderConfig {
    indent: String,
    reveal: bool,
    highlight: bool,
}

fn hl(config: &RenderConfig, s: &str, color: &str) -> String {
    if config.highlight && !s.trim().is_empty() {
        format!("{color}{s}{}", colors::RESET)
    } else {
        s.to_string()
    }
}

fn highlight_string(config: &RenderConfig, segment: &str) -> String {
    if !segment.contains('\\') {
        return hl(config, segment, highlights::STRING);
    }
    let chars: Vec<char> = segment.chars().collect();
    let mut result = String::new();
    let mut idx = 0usize;
    loop {
        let next_escape = chars[idx..]
            .iter()
            .position(|&c| c == '\\')
            .map(|p| idx + p)
            .unwrap_or(chars.len());
        let plain: String = chars[idx..next_escape].iter().collect();
        result.push_str(&hl(config, &plain, highlights::STRING));
        idx = next_escape;
        if chars.get(idx) != Some(&'\\') {
            break;
        }
        result.push_str(&hl(config, "\\", highlights::BRACKETS));
        idx += 1;
        let escaped = chars.get(idx).copied();
        let escaped_str = escaped.map(|c| c.to_string()).unwrap_or_default();
        if let Some(def) = escaped.and_then(unescape_def) {
            let color = if Some(def) == escaped {
                highlights::STRING
            } else {
                highlights::NUMBER
            };
            result.push_str(&hl(config, &escaped_str, color));
            idx += 1;
        } else if escaped == Some('x') {
            result.push_str(&hl(config, "x", highlights::KEYWORD));
            idx += 1;
            let d1 = chars.get(idx).map(|c| c.to_string()).unwrap_or_default();
            result.push_str(&hl(config, &d1, highlights::NUMBER));
            idx += 1;
            let d2 = chars.get(idx).map(|c| c.to_string()).unwrap_or_default();
            result.push_str(&hl(config, &d2, highlights::NUMBER));
        } else if escaped == Some('u') {
            result.push_str(&hl(config, "u", highlights::KEYWORD));
            idx += 1;
        } else {
            result.push_str(&hl(config, &escaped_str, highlights::ERROR));
            idx += 1;
        }
    }
    result
}

fn bracket_highlight(tag: BracketTag) -> Option<&'static str> {
    match tag {
        BracketTag::String => Some(highlights::BRACKETS),
        BracketTag::ColonCall => Some(highlights::OPERATORS),
        BracketTag::Map => Some(highlights::BRACKETS),
        BracketTag::List => Some(highlights::BRACKETS),
        BracketTag::Code => Some(highlights::BRACKETS),
        BracketTag::ArrowFn => Some(highlights::KEYWORD),
        BracketTag::InlineComment => Some(highlights::BRACKETS),
        BracketTag::None => None,
    }
}

fn op_highlight(tag: OpTag) -> Option<&'static str> {
    match tag {
        OpTag::Def => Some(highlights::KEYWORD),
        OpTag::Pub => Some(highlights::KEYWORD),
        OpTag::Assign => Some(highlights::KEYWORD),
        OpTag::Sep => Some(highlights::OPERATORS),
        OpTag::Var => Some(highlights::KEYWORD),
        OpTag::None => None,
    }
}

fn raw_highlight(tag: RawTag) -> Option<&'static str> {
    match tag {
        RawTag::Return => Some(highlights::KEYWORD),
        RawTag::Discard => Some(highlights::KEYWORD),
        RawTag::String => Some(highlights::STRING),
        RawTag::Comment => Some(highlights::COMMENT),
        RawTag::Void => None,
    }
}

fn ident_prefix_highlight(tag: IdentifierTag) -> Option<&'static str> {
    match tag {
        IdentifierTag::Access => Some(highlights::OPERATORS),
        IdentifierTag::Builtin => Some(highlights::BUILTIN),
        IdentifierTag::Normal | IdentifierTag::Number | IdentifierTag::Discard => None,
    }
}

fn ident_value_highlight(tag: IdentifierTag) -> Option<&'static str> {
    match tag {
        IdentifierTag::Normal => Some(highlights::IDENT),
        IdentifierTag::Access => Some(highlights::IDENT),
        IdentifierTag::Builtin => Some(highlights::BUILTIN),
        IdentifierTag::Number => Some(highlights::NUMBER),
        IdentifierTag::Discard => None,
    }
}

fn flatten_op_segs(entities: &[SyntaxNode]) -> Vec<&SyntaxNode> {
    let mut out = Vec::with_capacity(entities.len());
    for entity in entities {
        if let SyntaxNode::OperatorSegment(seg) = entity {
            out.extend(seg.items.iter());
        } else {
            out.push(entity);
        }
    }
    out
}

fn is_nl(entity: &SyntaxNode) -> bool {
    match entity {
        SyntaxNode::Whitespace(w) => w.nl,
        SyntaxNode::Operator(o) => o.op == "\n",
        _ => false,
    }
}

const RAINBOW: [&str; 6] = [
    colors::RED,
    colors::YELLOW,
    colors::GREEN,
    colors::CYAN,
    colors::BLUE,
    colors::MAGENTA,
];

fn render_entity_pretty_list(
    config: &RenderConfig,
    entities: &[SyntaxNode],
    indent: usize,
    depth: usize,
    is_top_level: bool,
) -> String {
    let entities = flatten_op_segs(entities);
    let mut result = String::new();
    let mut needs_deeper_indent = false;
    let mut did_insert_newline = false;
    let mut last_newline_index: i64 = -1;

    for (i, entity) in entities.iter().enumerate() {
        if is_nl(entity) {
            last_newline_index = i as i64;
        }
    }

    let reveal_color = RAINBOW[depth % RAINBOW.len()];
    if config.reveal {
        result.push_str(reveal_color);
        result.push('<');
        result.push_str(colors::RESET);
    }

    for (i, entity) in entities.iter().enumerate() {
        if is_nl(entity) {
            if !did_insert_newline {
                needs_deeper_indent = !is_top_level && (i as i64) < last_newline_index;
                did_insert_newline = true;
                result.push('\n');
                let depth_indent = indent + if needs_deeper_indent { 1 } else { 0 };
                result.push_str(&config.indent.repeat(depth_indent));
            }
        } else {
            did_insert_newline = false;
            let child_indent = indent + if needs_deeper_indent { 1 } else { 0 };
            result.push_str(&render_entity_pretty(
                config,
                entity,
                child_indent,
                depth + 1,
                is_top_level,
            ));
        }
    }
    if config.reveal {
        result.push_str(reveal_color);
        result.push('>');
        result.push_str(colors::RESET);
    }
    result
}

fn render_end_pretty(end: &str) -> &str {
    if end == IN_STRING_QUOTE {
        "\""
    } else if end == IN_INLINE_COMMENT_END {
        ""
    } else {
        end
    }
}

fn render_entity_pretty(
    config: &RenderConfig,
    entity: &SyntaxNode,
    indent: usize,
    depth: usize,
    is_top_level: bool,
) -> String {
    match entity {
        SyntaxNode::Block(b) => {
            let color = bracket_highlight(b.tag).unwrap_or(highlights::ERROR);
            format!(
                "{}{}{}",
                hl(config, &b.start, color),
                render_entity_pretty_list(config, &b.items, indent, depth, false),
                hl(config, render_end_pretty(&b.end), color)
            )
        }
        SyntaxNode::BinaryExpression(b) => {
            render_entity_pretty_list(config, &b.items, indent, depth, is_top_level)
        }
        SyntaxNode::Whitespace(w) => {
            if w.nl {
                String::new()
            } else {
                " ".to_string()
            }
        }
        SyntaxNode::Identifier(t) => {
            let prefix_color = ident_prefix_highlight(t.ident_tag).unwrap_or(highlights::ERROR);
            let value_color = ident_value_highlight(t.ident_tag).unwrap_or(highlights::ERROR);
            format!(
                "{}{}",
                hl(config, &t.ident_tag_raw, prefix_color),
                hl(config, &t.str, value_color)
            )
        }
        SyntaxNode::Operator(o) => {
            if o.op == "\n" {
                String::new()
            } else {
                hl(
                    config,
                    &o.op,
                    op_highlight(o.op_tag).unwrap_or(highlights::ERROR),
                )
            }
        }
        SyntaxNode::OperatorSegment(_) => {
            unreachable!("opSeg should be handled by renderEntityPrettyList")
        }
        SyntaxNode::Raw(r) => {
            if r.tag == RawTag::String && config.highlight {
                highlight_string(config, &r.raw)
            } else {
                hl(
                    config,
                    &r.raw,
                    raw_highlight(r.tag).unwrap_or(highlights::ERROR),
                )
            }
        }
        SyntaxNode::Err(_) => "%TODO<err>%".to_string(),
    }
}

pub mod colors {
    pub const BLACK: &str = "\x1b[30m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE: &str = "\x1b[37m";

    pub const BRBLACK: &str = "\x1b[90m";
    pub const BRRED: &str = "\x1b[91m";
    pub const BRGREEN: &str = "\x1b[92m";
    pub const BRYELLOW: &str = "\x1b[93m";
    pub const BRBLUE: &str = "\x1b[94m";
    pub const BRMAGENTA: &str = "\x1b[95m";
    pub const BRCYAN: &str = "\x1b[96m";
    pub const BRWHITE: &str = "\x1b[97m";

    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const ITALIC: &str = "\x1b[3m";
    pub const UNDERLINE: &str = "\x1b[4m";
    pub const BLINK: &str = "\x1b[5m";
    pub const INVERSE: &str = "\x1b[7m";
    pub const HIDDEN: &str = "\x1b[8m";
    pub const STRIKETHROUGH: &str = "\x1b[9m";
}

pub mod highlights {
    use super::colors;

    pub const STRING: &str = colors::GREEN;
    pub const KEYWORD: &str = colors::BLUE;
    pub const BRACKETS: &str = colors::BRBLACK;
    pub const OPERATORS: &str = colors::BRBLACK;
    pub const BUILTIN: &str = colors::CYAN;
    pub const COMMENT: &str = colors::YELLOW;
    pub const NUMBER: &str = colors::MAGENTA;
    pub const ERROR: &str = "\x1b[7m\x1b[31m";
    pub const IDENT: &str = "";
}

pub fn pretty_print_errors(sources: &[&Source], errors: &[TokenizationError]) -> String {
    if errors.is_empty() {
        return String::new();
    }

    let mut source_map: HashMap<&str, Option<&Source>> = HashMap::new();
    for source in sources {
        if source_map.contains_key(source.filename.as_str()) {
            source_map.insert(source.filename.as_str(), None);
        } else {
            source_map.insert(source.filename.as_str(), Some(source));
        }
    }

    let mut output = String::new();

    for error in errors {
        output.push('\n');

        for entry in &error.entries {
            let color = match entry.style {
                ErrorStyle::Error => colors::RED,
                ErrorStyle::Note => colors::CYAN,
                ErrorStyle::Todo => colors::BLUE,
                ErrorStyle::Warning => colors::YELLOW,
                ErrorStyle::Unreachable => colors::BRBLACK,
            };
            let bold = if entry.style != ErrorStyle::Note {
                colors::BOLD
            } else {
                ""
            };
            let style_str = match entry.style {
                ErrorStyle::Error => "error",
                ErrorStyle::Note => "note",
                ErrorStyle::Todo => "todo",
                ErrorStyle::Warning => "warning",
                ErrorStyle::Unreachable => "unreachable",
            };

            let fyl = entry.pos.as_ref().map(|p| p.fyl.as_str()).unwrap_or("??");
            let lyn = entry
                .pos
                .as_ref()
                .map(|p| p.lyn.to_string())
                .unwrap_or_else(|| "??".to_string());
            let col = entry
                .pos
                .as_ref()
                .map(|p| p.col.to_string())
                .unwrap_or_else(|| "??".to_string());

            output.push_str(&format!(
                "{fyl}:{lyn}:{col}: {color}{bold}{style_str}{reset}: {message}{reset}\n",
                reset = colors::RESET,
                message = entry.message,
            ));

            let source = entry.pos.as_ref().and_then(|p| {
                if p.fyl.is_empty() {
                    None
                } else {
                    source_map.get(p.fyl.as_str()).copied().flatten()
                }
            });
            let source_lines: Option<Vec<&str>> = source.map(|s| s.text.split('\n').collect());
            let line: Option<&str> = match (&source_lines, &entry.pos) {
                (Some(lines), Some(pos)) => {
                    pos.lyn.checked_sub(1).and_then(|i| lines.get(i)).copied()
                }
                _ => Some(""),
            };
            let Some(line) = line else { continue };

            let gutter_width = lyn.len();
            let empty_gutter = format!(
                " {} {}|{}",
                " ".repeat(gutter_width),
                colors::BLUE,
                colors::RESET
            );
            let line_gutter = format!(
                " {}{lyn}{} {}|{}",
                colors::CYAN,
                colors::RESET,
                colors::BLUE,
                colors::RESET
            );

            output.push_str(&format!("{line_gutter} {line}\n"));

            let pointer_col = entry.pos.as_ref().map(|p| p.col).unwrap_or(1);
            let pointer = format!("{}^", " ".repeat(pointer_col.saturating_sub(1)));
            output.push_str(&format!(
                "{empty_gutter} {color}{}{pointer}{}\n",
                colors::BOLD,
                colors::RESET
            ));
        }
        if !error.trace.is_empty() {
            for line in &error.trace {
                output.push_str(&format!(
                    " at {}:{}:{} ({})\n",
                    line.pos.fyl, line.pos.lyn, line.pos.col, line.text
                ));
            }
        }
    }

    output
}

pub fn render_tokenized_output(
    tokenization_result: &TokenizationResult,
    source: &Source,
) -> String {
    let formatted_code = render_entity_pretty_list(
        &RenderConfig {
            indent: "  ".to_string(),
            reveal: false,
            highlight: true,
        },
        &tokenization_result.result,
        0,
        0,
        true,
    );
    let ugly_code = render_entity_pretty_list(
        &RenderConfig {
            indent: "  ".to_string(),
            reveal: true,
            highlight: false,
        },
        &tokenization_result.result,
        0,
        0,
        true,
    );
    let adisp =
        crate::printers::printers::AST_NODE.dump_list(&tokenization_result.result, usize::MAX);
    let pretty_errors = pretty_print_errors(&[source], &tokenization_result.errors);

    format!(
        "// adisp:{adisp}\n\n// ugly\n{ugly_code}\n\n// formatted\n{formatted_code}\n\n// errors:\n{pretty_errors}"
    )
}
