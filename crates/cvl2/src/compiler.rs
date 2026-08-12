use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use crate::parser::{
    tokenize, BracketTag, ErrToken, ErrorStyle, IdentifierTag, OpTag, OperatorSegmentToken,
    OperatorToken, Source, SyntaxNode, TokenPosition, TokenizationError, TokenizationErrorEntry,
};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub struct PositionedError {
    pub e: TokenizationError,
}

impl std::fmt::Display for PositionedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut lines = Vec::new();
        for entry in &self.e.entries {
            let fyl = entry.pos.as_ref().map(|p| p.fyl.as_str()).unwrap_or("???");
            let lyn = entry
                .pos
                .as_ref()
                .map(|p| p.lyn.to_string())
                .unwrap_or_else(|| "???".to_string());
            let col = entry
                .pos
                .as_ref()
                .map(|p| p.col.to_string())
                .unwrap_or_else(|| "???".to_string());
            let style = match entry.style {
                ErrorStyle::Error => "error",
                ErrorStyle::Note => "note",
            };
            lines.push(format!("{fyl}:{lyn}:{col}: {style}: {}", entry.message));
        }
        for trace in &self.e.trace {
            lines.push(format!(
                " at {}:{}:{} ({})",
                trace.pos.fyl, trace.pos.lyn, trace.pos.col, trace.text
            ));
        }
        write!(f, "{}", lines.join("\n"))
    }
}

impl std::error::Error for PositionedError {}

fn compiler_pos() -> TokenPosition {
    TokenPosition {
        fyl: "compiler".to_string(),
        lyn: 0,
        col: 0,
        idx: 0,
    }
}

pub struct Env {
    pub trace: Vec<crate::parser::TraceEntry>,
    pub errors: Vec<TokenizationError>,
    pub comptime: HashMap<Symbol, Box<dyn Any>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetEnv {
    Comptime,
    Todo,
}

fn target_env_symbol() -> Symbol {
    static TARGET_ENV_SYMBOL: OnceLock<Symbol> = OnceLock::new();
    *TARGET_ENV_SYMBOL.get_or_init(Symbol::new)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Symbol(u64);

impl Symbol {
    pub fn new() -> Self {
        static NEXT_SYMBOL_ID: AtomicU64 = AtomicU64::new(0);
        Symbol(NEXT_SYMBOL_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for Symbol {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NsKey {
    Str(String),
    Sym(Symbol),
}

pub trait ComptimeNamespace {
    fn get_string(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        field: &str,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError>;
    fn get_symbol(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        keychild: ComptimeType,
        field: Symbol,
        block: &mut AnalysisBlock,
    ) -> Result<Option<AnalysisResult>, PositionedError>;
}

pub struct RegisteredEntry {
    pub key: ComptimeNarrowKey,
    pub ast: ComptimeValueAst,
}

pub struct NsFields {
    pub locked: bool,
    pub registered: HashMap<NsKey, RegisteredEntry>,
}

struct NamespaceImpl {
    fields: NsFields,
}

impl ComptimeNamespace for NamespaceImpl {
    fn get_string(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        field: &str,
        _block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let value = self.fields.registered.get(&NsKey::Str(field.to_string()));
        if value.is_some() {
            return Err(throw_err(env, Some(pos), "todo get registered field", None));
        }
        Err(throw_err(
            env,
            Some(pos.clone()),
            format!("string field '{field}' is not defined on namespace"),
            Some(vec![(Some(pos), "namespace declared here".to_string())]),
        ))
    }

    fn get_symbol(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        childt: ComptimeType,
        field: Symbol,
        block: &mut AnalysisBlock,
    ) -> Result<Option<AnalysisResult>, PositionedError> {
        let value = self.fields.registered.get(&NsKey::Sym(field));
        if let Some(entry) = value {
            let mut inner_block = AnalysisBlock { lines: Vec::new() };
            analyze(
                env,
                childt,
                entry.ast.pos.clone(),
                &entry.ast.ast,
                &mut inner_block,
            )?;
            let _ = block;
            return Err(throw_err(
                env,
                Some(pos),
                "todo handle analyzed result",
                None,
            ));
        }
        Ok(None)
    }
}

struct BuiltinNamespace;

impl ComptimeNamespace for BuiltinNamespace {
    fn get_string(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        field: &str,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        if field == "main" {
            let idx = block_append(
                block,
                AnalysisLine::ComptimeKey {
                    pos,
                    narrow: main_symbol_narrow(),
                },
            );
            Ok(AnalysisResult {
                idx,
                ty: ComptimeType::Key(main_symbol_type()),
            })
        } else {
            Err(throw_err(
                env,
                Some(pos),
                format!("builtin does not have field: {field}"),
                None,
            ))
        }
    }

    fn get_symbol(
        &self,
        _env: &mut Env,
        _pos: TokenPosition,
        _keychild: ComptimeType,
        _field: Symbol,
        _block: &mut AnalysisBlock,
    ) -> Result<Option<AnalysisResult>, PositionedError> {
        Ok(None)
    }
}

fn builtin_namespace() -> BuiltinNamespace {
    BuiltinNamespace
}

#[derive(Clone)]
pub struct ComptimeTypeVoid {
    pub pos: TokenPosition,
}
#[derive(Clone)]
pub struct ComptimeTypeKey {
    pub pos: TokenPosition,
    pub narrow: Option<ComptimeNarrowKey>,
}
#[derive(Clone)]
pub struct ComptimeTypeAst {
    pub pos: TokenPosition,
}
#[derive(Clone)]
pub struct ComptimeTypeUnknown {
    pub pos: TokenPosition,
}
#[derive(Clone)]
pub struct ComptimeTypeType {
    pub pos: TokenPosition,
    pub narrow: Option<Box<ComptimeType>>,
}
#[derive(Clone)]
pub struct ComptimeTypeNamespace {
    pub pos: TokenPosition,
    pub narrow: Option<Rc<dyn ComptimeNamespace>>,
}
#[derive(Clone)]
pub struct ComptimeTypeFn {
    pub pos: TokenPosition,
    pub arg: Box<ComptimeType>,
    pub ret: Box<ComptimeType>,
}
#[derive(Clone)]
pub struct ComptimeTypeFolderOrFile {
    pub pos: TokenPosition,
}
#[derive(Clone)]
pub struct ComptimeTypeTuple {
    pub pos: TokenPosition,
    pub children: Vec<ComptimeType>,
}

#[derive(Clone)]
pub enum ComptimeType {
    Void(ComptimeTypeVoid),
    Key(ComptimeTypeKey),
    Ast(ComptimeTypeAst),
    Unknown(ComptimeTypeUnknown),
    Type(ComptimeTypeType),
    Namespace(ComptimeTypeNamespace),
    Fn(ComptimeTypeFn),
    FolderOrFile(ComptimeTypeFolderOrFile),
    Tuple(ComptimeTypeTuple),
}

impl ComptimeType {
    pub fn pos(&self) -> &TokenPosition {
        match self {
            ComptimeType::Void(t) => &t.pos,
            ComptimeType::Key(t) => &t.pos,
            ComptimeType::Ast(t) => &t.pos,
            ComptimeType::Unknown(t) => &t.pos,
            ComptimeType::Type(t) => &t.pos,
            ComptimeType::Namespace(t) => &t.pos,
            ComptimeType::Fn(t) => &t.pos,
            ComptimeType::FolderOrFile(t) => &t.pos,
            ComptimeType::Tuple(t) => &t.pos,
        }
    }

    pub fn tag(&self) -> &'static str {
        match self {
            ComptimeType::Void(_) => "void",
            ComptimeType::Key(_) => "key",
            ComptimeType::Ast(_) => "ast",
            ComptimeType::Unknown(_) => "unknown",
            ComptimeType::Type(_) => "type",
            ComptimeType::Namespace(_) => "namespace",
            ComptimeType::Fn(_) => "fn",
            ComptimeType::FolderOrFile(_) => "folder_or_file",
            ComptimeType::Tuple(_) => "tuple",
        }
    }
}

#[derive(Clone)]
pub enum ComptimeNarrowKey {
    Symbol {
        key: Symbol,
        child: Box<ComptimeType>,
    },
    String {
        key: String,
    },
}

#[derive(Clone)]
pub struct ComptimeValueAst {
    pub ast: Vec<SyntaxNode>,
    pub pos: TokenPosition,
}

pub enum AnalysisLine {
    ComptimeOnly {
        pos: TokenPosition,
    },
    ComptimeNsListInit {
        pos: TokenPosition,
    },
    ComptimeKey {
        pos: TokenPosition,
        narrow: ComptimeNarrowKey,
    },
    ComptimeAst {
        pos: TokenPosition,
        narrow: ComptimeValueAst,
    },
    ComptimeNsListAppend {
        pos: TokenPosition,
        key: BlockIdx,
        list: BlockIdx,
        value: BlockIdx,
    },
    Void {
        pos: TokenPosition,
    },
    Call {
        pos: TokenPosition,
        method: BlockIdx,
        arg: BlockIdx,
    },
    Break {
        pos: TokenPosition,
        value: BlockIdx,
    },
}

pub struct AnalysisBlock {
    pub lines: Vec<AnalysisLine>,
}

pub struct AnalysisResult {
    pub idx: BlockIdx,
    pub ty: ComptimeType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockIdx(pub usize);

pub fn block_append(block: &mut AnalysisBlock, instr: AnalysisLine) -> BlockIdx {
    block.lines.push(instr);
    BlockIdx(block.lines.len() - 1)
}

pub fn analyze_call(
    env: &mut Env,
    _slot: ComptimeType,
    pos: TokenPosition,
    method: AnalysisResult,
    get_arg: &mut dyn FnMut(
        &mut Env,
        ComptimeType,
        TokenPosition,
        &mut AnalysisBlock,
    ) -> AnalysisResult,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    if let ComptimeType::Fn(f) = &method.ty {
        let arg_slot = (*f.arg).clone();
        let ret_ty = (*f.ret).clone();
        let arg = get_arg(env, arg_slot, pos.clone(), block);
        let idx = block_append(
            block,
            AnalysisLine::Call {
                pos,
                method: method.idx,
                arg: arg.idx,
            },
        );
        Ok(AnalysisResult { idx, ty: ret_ty })
    } else {
        Err(throw_err(
            env,
            Some(pos),
            format!("not supported call type: {}", method.ty.tag()),
            None,
        ))
    }
}

pub fn analyze(
    env: &mut Env,
    slot: ComptimeType,
    pos: TokenPosition,
    ast: &[SyntaxNode],
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    if let ComptimeType::Ast(_) = &slot {
        let value = ComptimeValueAst {
            ast: ast.to_vec(),
            pos: pos.clone(),
        };
        let idx = block_append(
            block,
            AnalysisLine::ComptimeAst {
                pos: pos.clone(),
                narrow: value,
            },
        );
        return Ok(AnalysisResult {
            idx,
            ty: ComptimeType::Ast(ComptimeTypeAst { pos }),
        });
    }
    let ast = trim_ws(ast);

    if ast.is_empty() {
        return Err(throw_err(
            env,
            Some(pos),
            "failed to analyze empty expression",
            None,
        ));
    }

    let last = ast.len() - 1;
    analyze_sub(env, slot.clone(), slot, &ast, last, block)
}

pub fn analyze_sub(
    env: &mut Env,
    slot: ComptimeType,
    root_slot: ComptimeType,
    ast: &[SyntaxNode],
    index: usize,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let expr = &ast[index];

    if let SyntaxNode::Identifier(id) = expr {
        if id.ident_tag == IdentifierTag::Access {
            let unknown_slot = ComptimeType::Unknown(ComptimeTypeUnknown {
                pos: compiler_pos(),
            });
            let lhs = if index >= 1 {
                analyze_sub(env, unknown_slot, root_slot.clone(), ast, index - 1, block)?
            } else {
                let idx = block_append(
                    block,
                    AnalysisLine::ComptimeOnly {
                        pos: id.pos.clone(),
                    },
                );
                AnalysisResult {
                    idx,
                    ty: ComptimeType::Type(ComptimeTypeType {
                        pos: slot.pos().clone(),
                        narrow: Some(Box::new(root_slot.clone())),
                    }),
                }
            };
            let narrow = ComptimeNarrowKey::String {
                key: id.str.clone(),
            };
            let idx = block_append(
                block,
                AnalysisLine::ComptimeKey {
                    pos: id.pos.clone(),
                    narrow: narrow.clone(),
                },
            );
            let prop = AnalysisResult {
                idx,
                ty: ComptimeType::Key(ComptimeTypeKey {
                    pos: id.pos.clone(),
                    narrow: Some(narrow),
                }),
            };
            return analyze_access(env, slot, lhs, id.pos.clone(), prop, block);
        }
    } else if let SyntaxNode::Block(b) = expr {
        if b.tag == BracketTag::ArrowFn {
            read_destructure(env, b.pos.clone(), &ast[..index])?;
            return Err(throw_err(
                env,
                Some(b.pos.clone()),
                "TODO: analyze the function now",
                None,
            ));
        }
    }

    if index == 0 {
        analyze_base(env, slot, expr, block)
    } else {
        Err(throw_err(
            env,
            Some(syntax_node_pos(expr).clone()),
            format!(
                "TODO analyzeSuffix: {}{}",
                syntax_node_kind(expr),
                dump_ast_node_list(std::slice::from_ref(expr), 2)
            ),
            None,
        ))
    }
}

fn std_folder_or_file_type() -> ComptimeTypeFolderOrFile {
    ComptimeTypeFolderOrFile {
        pos: compiler_pos(),
    }
}

fn main_symbol() -> Symbol {
    static MAIN_SYMBOL: OnceLock<Symbol> = OnceLock::new();
    *MAIN_SYMBOL.get_or_init(Symbol::new)
}

fn main_symbol_child_type() -> ComptimeType {
    ComptimeType::Fn(ComptimeTypeFn {
        pos: compiler_pos(),
        arg: Box::new(ComptimeType::Void(ComptimeTypeVoid {
            pos: compiler_pos(),
        })),
        ret: Box::new(ComptimeType::FolderOrFile(std_folder_or_file_type())),
    })
}

fn main_symbol_narrow() -> ComptimeNarrowKey {
    ComptimeNarrowKey::Symbol {
        key: main_symbol(),
        child: Box::new(main_symbol_child_type()),
    }
}

fn main_symbol_type() -> ComptimeTypeKey {
    ComptimeTypeKey {
        pos: compiler_pos(),
        narrow: Some(main_symbol_narrow()),
    }
}

pub fn analyze_base(
    env: &mut Env,
    _slot: ComptimeType,
    ast: &SyntaxNode,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    if let SyntaxNode::Identifier(id) = ast {
        if id.ident_tag == IdentifierTag::Builtin {
            if id.str == "builtin" {
                let idx = block_append(
                    block,
                    AnalysisLine::ComptimeOnly {
                        pos: id.pos.clone(),
                    },
                );
                return Ok(AnalysisResult {
                    idx,
                    ty: ComptimeType::Namespace(ComptimeTypeNamespace {
                        pos: compiler_pos(),
                        narrow: Some(Rc::new(builtin_namespace())),
                    }),
                });
            } else {
                return Err(throw_err(
                    env,
                    Some(id.pos.clone()),
                    format!("unexpected builtin: #{}", id.str),
                    None,
                ));
            }
        }
    }
    Err(throw_err(
        env,
        Some(syntax_node_pos(ast).clone()),
        format!(
            "TODO analyzeBase: {}{}",
            syntax_node_kind(ast),
            dump_ast_node_list(std::slice::from_ref(ast), 3)
        ),
        None,
    ))
}

pub fn analyze_access(
    env: &mut Env,
    _slot: ComptimeType,
    obj: AnalysisResult,
    pos: TokenPosition,
    prop: AnalysisResult,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    if let ComptimeType::Namespace(ns) = &obj.ty {
        let Some(ns_impl) = ns.narrow.clone() else {
            return Err(throw_err(
                env,
                Some(pos),
                "cannot access on non-narrowed namespace",
                None,
            ));
        };
        let ComptimeType::Key(key_ty) = &prop.ty else {
            return Err(throw_err(env, Some(pos), "expected prop type key", None));
        };
        let Some(narrow) = key_ty.narrow.clone() else {
            return Err(throw_err(
                env,
                Some(pos),
                "cannot access on namespace with non-narrowed prop value",
                None,
            ));
        };
        return match narrow {
            ComptimeNarrowKey::String { key } => ns_impl.get_string(env, pos, &key, block),
            ComptimeNarrowKey::Symbol { .. } => Err(throw_err(
                env,
                Some(pos),
                "TODO return ?symbolChildType .init(T) or .empty",
                None,
            )),
        };
    }
    Err(throw_err(
        env,
        Some(pos),
        format!("TODO: analyze access on type: {}", obj.ty.tag()),
        None,
    ))
}

pub struct ReadBinding {
    pub pos: TokenPosition,
    pub value: Vec<SyntaxNode>,
}

pub struct ReadContainer {
    pub bindings: HashMap<String, ReadBinding>,
    pub lines: Vec<OperatorSegmentToken>,
}

#[derive(Clone)]
pub struct Destructure {
    pub extract: DestructureExtract,
    pub ty: ComptimeType,
}

#[derive(Clone)]
pub enum DestructureExtract {
    SingleItem {
        name: String,
        pos: TokenPosition,
    },
    List {
        items: Vec<DestructureExtract>,
        pos: TokenPosition,
    },
    Map {
        items: Vec<(ComptimeNarrowKey, DestructureExtract)>,
        pos: TokenPosition,
    },
}

impl DestructureExtract {
    pub fn pos(&self) -> &TokenPosition {
        match self {
            DestructureExtract::SingleItem { pos, .. } => pos,
            DestructureExtract::List { pos, .. } => pos,
            DestructureExtract::Map { pos, .. } => pos,
        }
    }

    pub fn tag(&self) -> &'static str {
        match self {
            DestructureExtract::SingleItem { .. } => "single_item",
            DestructureExtract::List { .. } => "list",
            DestructureExtract::Map { .. } => "map",
        }
    }
}

pub fn read_destructure(
    env: &mut Env,
    pos: TokenPosition,
    src: &[SyntaxNode],
) -> Result<Destructure, PositionedError> {
    let lhs_items = trim_ws(src);
    if lhs_items.is_empty() {
        return Err(throw_err(
            env,
            Some(pos),
            format!(
                "Expected at least one item to destructure{}",
                dump_ast_node_list(src, 2)
            ),
            None,
        ));
    }
    if lhs_items.len() > 1 {
        return Err(throw_err(
            env,
            Some(syntax_node_pos(&lhs_items[1]).clone()),
            format!(
                "Unexpected item for destructuring. TODO support eg 'name: type := value'{}",
                dump_ast_node_list(src, 2)
            ),
            None,
        ));
    }
    let ident = &lhs_items[0];
    match ident {
        SyntaxNode::Identifier(id) if id.ident_tag == IdentifierTag::Normal => Ok(Destructure {
            extract: DestructureExtract::SingleItem {
                name: id.str.clone(),
                pos: id.pos.clone(),
            },
            ty: ComptimeType::Unknown(ComptimeTypeUnknown {
                pos: id.pos.clone(),
            }),
        }),
        SyntaxNode::Block(b) if b.tag == BracketTag::List => {
            let args = read_binary(env, b.pos.clone(), &b.items, OpTag::Sep)?;
            let mut extracts = Vec::new();
            let mut types = Vec::new();
            for arg in args {
                if arg.items.is_empty() {
                    continue;
                }
                let sub = read_destructure(env, arg.pos.clone(), &arg.items)?;
                extracts.push(sub.extract);
                types.push(sub.ty);
            }
            Ok(Destructure {
                extract: DestructureExtract::List {
                    items: extracts,
                    pos: b.pos.clone(),
                },
                ty: ComptimeType::Tuple(ComptimeTypeTuple {
                    children: types,
                    pos: b.pos.clone(),
                }),
            })
        }
        _ => Err(throw_err(
            env,
            Some(syntax_node_pos(ident).clone()),
            format!(
                "Unsupported kind for destructuring: {}",
                syntax_node_kind(ident)
            ),
            None,
        )),
    }
}

fn read_container_line(
    env: &mut Env,
    res: &mut ReadContainer,
    line: &OperatorSegmentToken,
) -> Result<(), PositionedError> {
    if line.items.is_empty() {
        return Ok(());
    }
    let rb2 = read_binary2(env, line.pos.clone(), &line.items, OpTag::Def)?;
    if let Some((lhs, op, rhs)) = rb2 {
        let destructure = read_destructure(env, lhs.pos.clone(), &lhs.items)?;
        let DestructureExtract::SingleItem {
            name,
            pos: extract_pos,
        } = &destructure.extract
        else {
            return Err(throw_err(
                env,
                Some(destructure.extract.pos().clone()),
                format!(
                    "TODO: support destructure extract kind: {}",
                    destructure.extract.tag()
                ),
                None,
            ));
        };
        if let Some(prev) = res.bindings.get(name) {
            let prev_pos = prev.pos.clone();
            add_err(
                env,
                Some(extract_pos.clone()),
                format!("Duplicate binding name {name}"),
                Some(vec![(
                    Some(prev_pos.clone()),
                    "Previous definition here".to_string(),
                )]),
            );
            res.bindings.insert(
                name.clone(),
                ReadBinding {
                    pos: prev_pos.clone(),
                    value: vec![SyntaxNode::Err(ErrToken { pos: prev_pos })],
                },
            );
        } else {
            res.bindings.insert(
                name.clone(),
                ReadBinding {
                    pos: op.pos.clone(),
                    value: rhs.items.clone(),
                },
            );
        }
    } else {
        res.lines.push(line.clone());
    }
    Ok(())
}

pub fn read_container(
    env: &mut Env,
    pos: TokenPosition,
    src: &[SyntaxNode],
) -> Result<ReadContainer, PositionedError> {
    let lines = read_binary(env, pos, src, OpTag::Sep)?;
    let mut res = ReadContainer {
        bindings: HashMap::new(),
        lines: Vec::new(),
    };
    for line in &lines {
        if let Err(e) = read_container_line(env, &mut res, line) {
            env.errors.push(e.e);
        }
    }
    Ok(res)
}

pub fn trim_ws(src: &[SyntaxNode]) -> Vec<SyntaxNode> {
    src.iter()
        .filter(|item| !matches!(item, SyntaxNode::Whitespace(_)))
        .cloned()
        .collect()
}

pub type Binary2 = (OperatorSegmentToken, OperatorToken, OperatorSegmentToken);

pub fn read_binary2(
    env: &mut Env,
    pos: TokenPosition,
    root_src: &[SyntaxNode],
    kw: OpTag,
) -> Result<Option<Binary2>, PositionedError> {
    let root_src = trim_ws(root_src);
    if root_src.is_empty() {
        return Ok(None);
    }
    let SyntaxNode::BinaryExpression(bin) = &root_src[0] else {
        return Ok(None);
    };
    if bin.tag != kw {
        return Ok(None);
    }
    let src = trim_ws(&bin.items);
    if src.len() != 3 {
        return Err(throw_err(
            env,
            Some(pos),
            "Expected LHS op RHS, found not that",
            None,
        ));
    }
    match (&src[0], &src[1], &src[2]) {
        (
            SyntaxNode::OperatorSegment(lhs),
            SyntaxNode::Operator(op),
            SyntaxNode::OperatorSegment(rhs),
        ) => Ok(Some((lhs.clone(), op.clone(), rhs.clone()))),
        _ => Ok(None),
    }
}

pub fn read_binary(
    env: &mut Env,
    pos: TokenPosition,
    src: &[SyntaxNode],
    kw: OpTag,
) -> Result<Vec<OperatorSegmentToken>, PositionedError> {
    let src = trim_ws(src);
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let is_kw_binary = matches!(&src[0], SyntaxNode::BinaryExpression(b) if b.tag == kw);
    if !is_kw_binary {
        return Ok(vec![OperatorSegmentToken { pos, items: src }]);
    }
    if src.len() > 1 {
        return Err(throw_err(
            env,
            Some(syntax_node_pos(&src[1]).clone()),
            "Found extra trailing items while parsing readBinary",
            None,
        ));
    }
    let SyntaxNode::BinaryExpression(bin) = &src[0] else {
        unreachable!()
    };
    let mut out = Vec::new();
    for itm in &bin.items {
        match itm {
            SyntaxNode::OperatorSegment(seg) => out.push(seg.clone()),
            SyntaxNode::Operator(_) => {}
            other => {
                return Err(throw_err(
                    env,
                    Some(syntax_node_pos(other).clone()),
                    format!(
                        "Unexpected token in {}: {}",
                        op_tag_str(kw),
                        syntax_node_kind(other)
                    ),
                    None,
                ));
            }
        }
    }
    Ok(out)
}

fn op_tag_str(tag: OpTag) -> &'static str {
    match tag {
        OpTag::Sep => "sep",
        OpTag::Def => "def",
        OpTag::Pub => "pub",
        OpTag::Var => "var",
        OpTag::Assign => "assign",
        OpTag::None => "",
    }
}

fn syntax_node_pos(node: &SyntaxNode) -> &TokenPosition {
    match node {
        SyntaxNode::Identifier(t) => &t.pos,
        SyntaxNode::Whitespace(t) => &t.pos,
        SyntaxNode::Operator(t) => &t.pos,
        SyntaxNode::OperatorSegment(t) => &t.pos,
        SyntaxNode::Block(t) => &t.pos,
        SyntaxNode::BinaryExpression(t) => &t.pos,
        SyntaxNode::StrSeg(t) => &t.pos,
        SyntaxNode::Raw(t) => &t.pos,
        SyntaxNode::Err(t) => &t.pos,
    }
}

fn syntax_node_kind(node: &SyntaxNode) -> &'static str {
    match node {
        SyntaxNode::Identifier(_) => "ident",
        SyntaxNode::Whitespace(_) => "ws",
        SyntaxNode::Operator(_) => "op",
        SyntaxNode::OperatorSegment(_) => "opSeg",
        SyntaxNode::Block(_) => "block",
        SyntaxNode::BinaryExpression(_) => "binary",
        SyntaxNode::StrSeg(_) => "strSeg",
        SyntaxNode::Raw(_) => "raw",
        SyntaxNode::Err(_) => "err",
    }
}

pub fn analyze_block(
    env: &mut Env,
    _slot: ComptimeType,
    pos: TokenPosition,
    src: &[SyntaxNode],
    block: &mut AnalysisBlock,
    mut analyze_bind: impl FnMut(
        &mut Env,
        Binary2,
        &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError>,
) -> Result<AnalysisResult, PositionedError> {
    let container = read_container(env, pos.clone(), src)?;

    for line in &container.lines {
        let rb2 = read_binary2(env, line.pos.clone(), &line.items, OpTag::Pub)?;
        if let Some(b2) = rb2 {
            analyze_bind(env, b2, block)?;
        } else {
            analyze(
                env,
                ComptimeType::Void(ComptimeTypeVoid {
                    pos: line.pos.clone(),
                }),
                line.pos.clone(),
                &line.items,
                block,
            )?;
        }
    }

    let ret = block_append(block, AnalysisLine::Void { pos: pos.clone() });
    Ok(AnalysisResult {
        idx: ret,
        ty: ComptimeType::Void(ComptimeTypeVoid { pos }),
    })
}

fn analyze_namespace_bind(
    env: &mut Env,
    b2: Binary2,
    block: &mut AnalysisBlock,
    list: BlockIdx,
) -> Result<AnalysisResult, PositionedError> {
    let (lhs, op, rhs) = b2;
    let key = analyze(
        env,
        ComptimeType::Key(ComptimeTypeKey {
            pos: compiler_pos(),
            narrow: None,
        }),
        lhs.pos.clone(),
        &lhs.items,
        block,
    )?;
    let ComptimeType::Key(key_ty) = &key.ty else {
        panic!("unreachable")
    };
    if key_ty.narrow.is_none() {
        return Err(throw_err(
            env,
            Some(lhs.pos.clone()),
            "Expected narrowed key, got un-narrowed key",
            Some(vec![(
                None,
                "This error is unnecessary because we're not varying the slot type of the value based on the type of the key".to_string(),
            )]),
        ));
    }
    let value = analyze(
        env,
        ComptimeType::Ast(ComptimeTypeAst {
            pos: compiler_pos(),
        }),
        rhs.pos.clone(),
        &rhs.items,
        block,
    )?;
    let ret = block_append(
        block,
        AnalysisLine::ComptimeNsListAppend {
            pos: op.pos.clone(),
            list,
            key: key.idx,
            value: value.idx,
        },
    );
    Ok(AnalysisResult {
        idx: ret,
        ty: ComptimeType::Void(ComptimeTypeVoid {
            pos: compiler_pos(),
        }),
    })
}

pub fn analyze_namespace(
    env: &mut Env,
    pos: TokenPosition,
    src: &[SyntaxNode],
) -> Result<Rc<dyn ComptimeNamespace>, PositionedError> {
    let mut block = AnalysisBlock { lines: Vec::new() };
    let arr_entry = block_append(
        &mut block,
        AnalysisLine::ComptimeNsListInit { pos: pos.clone() },
    );

    analyze_block(
        env,
        ComptimeType::Void(ComptimeTypeVoid {
            pos: compiler_pos(),
        }),
        pos,
        src,
        &mut block,
        |env, b2, block| analyze_namespace_bind(env, b2, block, arr_entry),
    )?;

    let results = comptime_eval(env, &block);
    let arr_value_box = results
        .into_iter()
        .nth(arr_entry.0)
        .expect("comptime_eval must produce one result per block line");
    let mut arr_value = *arr_value_box
        .downcast::<NsFields>()
        .expect("comptime:ns_list_init result must be NsFields");
    arr_value.locked = true;

    Ok(Rc::new(NamespaceImpl { fields: arr_value }))
}

pub fn comptime_eval(_env: &mut Env, _block: &AnalysisBlock) -> Vec<Box<dyn Any>> {
    todo!("port cte.ts comptimeEval")
}

// cte.ts's `printers.astNode.dumpList` isn't ported yet; callers only use this
// to append a diagnostic dump to error messages, so an empty placeholder keeps
// those (otherwise fully working) error paths from panicking in the meantime.
fn dump_ast_node_list(_nodes: &[SyntaxNode], _depth: usize) -> String {
    String::new()
}

pub fn throw_err(
    env: &Env,
    pos: Option<TokenPosition>,
    msg: impl Into<String>,
    notes: Option<Vec<(Option<TokenPosition>, String)>>,
) -> PositionedError {
    PositionedError {
        e: get_err(env, pos, msg, notes),
    }
}

pub fn add_err(
    env: &mut Env,
    pos: Option<TokenPosition>,
    msg: impl Into<String>,
    notes: Option<Vec<(Option<TokenPosition>, String)>>,
) {
    let err = get_err(env, pos, msg, notes);
    env.errors.push(err);
}

pub fn get_err(
    env: &Env,
    pos: Option<TokenPosition>,
    msg: impl Into<String>,
    notes: Option<Vec<(Option<TokenPosition>, String)>>,
) -> TokenizationError {
    let mut entries = vec![TokenizationErrorEntry {
        pos,
        style: ErrorStyle::Error,
        message: msg.into(),
    }];
    for (note_pos, note_msg) in notes.into_iter().flatten() {
        entries.push(TokenizationErrorEntry {
            pos: note_pos,
            style: ErrorStyle::Note,
            message: note_msg,
        });
    }
    // JS's `Error().stack` reflection (`parseErrorStack`) has no Rust equivalent, so the
    // synthetic call-stack frames it produced are simply omitted here.
    TokenizationError {
        entries,
        trace: env.trace.clone(),
    }
}

pub fn assert(condition: bool) {
    if !condition {
        panic!("assertion failed");
    }
}

fn import_file_body(
    env: &mut Env,
    filename: &str,
    root_pos: TokenPosition,
    tokenized_result: &[SyntaxNode],
) -> Result<(), PositionedError> {
    let mut block = AnalysisBlock { lines: Vec::new() };
    let ns = analyze_namespace(
        env,
        TokenPosition {
            fyl: filename.to_string(),
            lyn: 0,
            col: 0,
            idx: 0,
        },
        tokenized_result,
    )?;
    let main_fn = ns.get_symbol(
        env,
        root_pos.clone(),
        main_symbol_child_type(),
        main_symbol(),
        &mut block,
    )?;
    let Some(main_fn) = main_fn else {
        return Err(throw_err(env, Some(root_pos), "expected main fn", None));
    };
    analyze_call(
        env,
        ComptimeType::FolderOrFile(std_folder_or_file_type()),
        root_pos,
        main_fn,
        &mut |_env, _slot, _pos, block| AnalysisResult {
            idx: block_append(
                block,
                AnalysisLine::Void {
                    pos: compiler_pos(),
                },
            ),
            ty: ComptimeType::Void(ComptimeTypeVoid {
                pos: compiler_pos(),
            }),
        },
        &mut block,
    )?;
    comptime_eval(env, &block);
    Ok(())
}

pub fn import_file(filename: &str, contents: &str) -> Result<(), Vec<TokenizationError>> {
    let mut source = Source::new(filename, contents);
    let tokenized = tokenize(&mut source);
    let root_pos = TokenPosition {
        fyl: filename.to_string(),
        lyn: 0,
        col: 0,
        idx: 0,
    };

    let mut env = Env {
        trace: Vec::new(),
        errors: tokenized.errors.clone(),
        comptime: HashMap::new(),
    };
    env.comptime
        .insert(target_env_symbol(), Box::new(TargetEnv::Comptime));

    if let Err(e) = import_file_body(&mut env, filename, root_pos, &tokenized.result) {
        env.errors.push(e.e);
    }

    if env.errors.is_empty() {
        Ok(())
    } else {
        Err(env.errors)
    }
}
