use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use crate::ct::{
    CExportName, CallArg, CtAst, CtBuildArtifact, CtBuildArtifactNarrow, CtExportList, CtKey,
    CtNamespace, CtType, McIdentifier, McNbtRef, McNbtRefNarrow, McResult, Type, TypeFn, TypeTuple,
    TypeUnknown, TypeVoid,
};
use crate::parser::{
    tokenize, BracketTag, IdentifierTag, OpTag, OperatorSegmentToken, OperatorToken, RawTag,
    RawToken, Source, SyntaxNode, TokenPosition, TokenizationError, TokenizationErrorEntry,
    TraceEntry,
};
use crate::printers::printers::AST_NODE;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub enum PositionedError {
    Fresh(TokenizationError),
    Consumed,
}

impl std::fmt::Display for PositionedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let e = match self {
            PositionedError::Fresh(e) => e,
            PositionedError::Consumed => return write!(f, "(error already reported)"),
        };
        let mut lines = Vec::new();
        for entry in &e.entries {
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
            lines.push(format!(
                "{fyl}:{lyn}:{col}: {:?}: {}",
                entry.style, entry.message
            ));
        }
        for trace in &e.trace {
            lines.push(format!(
                " at {}:{}:{} ({})",
                trace.pos.fyl, trace.pos.lyn, trace.pos.col, trace.text
            ));
        }
        write!(f, "{}", lines.join("\n"))
    }
}

impl std::error::Error for PositionedError {}

pub fn compiler_pos() -> TokenPosition {
    TokenPosition {
        fyl: "compiler".to_string(),
        lyn: 0,
        col: 0,
        idx: 0,
    }
}

pub fn empty_block() -> AnalysisBlock {
    AnalysisBlock {
        offset: 0,
        lines: Vec::new(),
        validate: Symbol::new(),
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsumedErrorToken;

pub fn throw_consumed_err(_consumed: ConsumedErrorToken) -> PositionedError {
    PositionedError::Consumed
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetEnv {
    Build,
    C,
    Mc,
    Todo,
}

fn target_env_symbol() -> Symbol {
    static TARGET_ENV_SYMBOL: OnceLock<Symbol> = OnceLock::new();
    *TARGET_ENV_SYMBOL.get_or_init(Symbol::new)
}

#[derive(Debug)]
pub struct ComptimeScopeMap {
    parent: Option<Rc<ComptimeScopeMap>>,
    changes: HashMap<Symbol, TargetEnv>,
}

impl ComptimeScopeMap {
    pub fn root(changes: HashMap<Symbol, TargetEnv>) -> Rc<Self> {
        Rc::new(ComptimeScopeMap {
            parent: None,
            changes,
        })
    }

    pub fn get(self: &Rc<Self>, key: Symbol) -> Option<TargetEnv> {
        if let Some(v) = self.changes.get(&key) {
            return Some(*v);
        }
        self.parent.as_ref().and_then(|p| p.get(key))
    }

    pub fn sub(self: &Rc<Self>, changes: HashMap<Symbol, TargetEnv>) -> Rc<Self> {
        Rc::new(ComptimeScopeMap {
            parent: Some(self.clone()),
            changes,
        })
    }
}

#[derive(Debug, Clone)]
pub enum Binding {
    Valid {
        pos: TokenPosition,
        decl: ComptimeValueDeclaration,
    },
    Runtime {
        pos: TokenPosition,
        runtime: AnalysisResult,
    },
    Error {
        pos: TokenPosition,
        consumed: ConsumedErrorToken,
    },
    Removed {
        pos: TokenPosition,
    },
}

fn binding_pos(binding: &Binding) -> &TokenPosition {
    match binding {
        Binding::Valid { pos, .. } => pos,
        Binding::Runtime { pos, .. } => pos,
        Binding::Error { pos, .. } => pos,
        Binding::Removed { pos } => pos,
    }
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub comptime: Rc<ComptimeScopeMap>,
    pub bindings: HashMap<String, Binding>,
}

pub trait CacheKey {
    fn cache_ptr(&self) -> usize;
}

pub struct PerComptimeScopeCache<K, V> {
    entries: RefCell<HashMap<usize, V>>,
    in_progress: RefCell<HashSet<usize>>,
    _marker: PhantomData<K>,
}

impl<K, V> Default for PerComptimeScopeCache<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> PerComptimeScopeCache<K, V> {
    pub fn new() -> Self {
        PerComptimeScopeCache {
            entries: RefCell::new(HashMap::new()),
            in_progress: RefCell::new(HashSet::new()),
            _marker: PhantomData,
        }
    }
}

impl<K: CacheKey, V: Clone> PerComptimeScopeCache<K, V> {
    pub fn get_or_put(
        &self,
        key: &K,
        env: &mut Env,
        cb: impl FnOnce(&mut Env) -> Result<V, PositionedError>,
    ) -> Result<V, PositionedError> {
        let ptr = key.cache_ptr();
        if let Some(v) = self.entries.borrow().get(&ptr) {
            return Ok(v.clone());
        }
        if !self.in_progress.borrow_mut().insert(ptr) {
            return Err(throw_err(env, None, "dependency loop", None, None));
        }
        match cb(env) {
            Ok(v) => {
                self.in_progress.borrow_mut().remove(&ptr);
                self.entries.borrow_mut().insert(ptr, v.clone());
                Ok(v)
            }
            Err(e) => Err(e),
        }
    }
}

pub struct Env {
    pub trace: Vec<TraceEntry>,
    pub errors: Vec<TokenizationError>,
    pub scope: Scope,
    pub fn_cache: Rc<PerComptimeScopeCache<ComptimeValueFn, AnalyzedFn>>,
    pub decl_cache: Rc<PerComptimeScopeCache<ComptimeValueDeclaration, ComptimeAnalysisResult>>,
    pub builtin_cache: Rc<PerComptimeScopeCache<Rc<dyn Descriptor>, AnalysisResult>>,
}

fn with_scope<R>(
    env: &mut Env,
    scope: Scope,
    f: impl FnOnce(&mut Env) -> Result<R, PositionedError>,
) -> Result<R, PositionedError> {
    let saved = std::mem::replace(&mut env.scope, scope);
    let result = f(env);
    env.scope = saved;
    result
}

fn with_target_env<R>(
    env: &mut Env,
    target: TargetEnv,
    f: impl FnOnce(&mut Env) -> Result<R, PositionedError>,
) -> Result<R, PositionedError> {
    let mut changes = HashMap::new();
    changes.insert(target_env_symbol(), target);
    let new_comptime = env.scope.comptime.sub(changes);
    let old_comptime = std::mem::replace(&mut env.scope.comptime, new_comptime);
    let result = f(env);
    env.scope.comptime = old_comptime;
    result
}

#[derive(Debug, Clone)]
pub struct ComptimeValueAst {
    pub ast: Vec<SyntaxNode>,
    pub pos: TokenPosition,
    pub scope: Scope,
}

#[derive(Debug)]
struct ComptimeValueDeclarationInner {
    ast: ComptimeValueAst,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueDeclaration(Rc<ComptimeValueDeclarationInner>);

impl ComptimeValueDeclaration {
    pub fn ast(&self) -> &ComptimeValueAst {
        &self.0.ast
    }
}

impl CacheKey for ComptimeValueDeclaration {
    fn cache_ptr(&self) -> usize {
        Rc::as_ptr(&self.0) as usize
    }
}

pub fn create_declaration(_env: &mut Env, ast: ComptimeValueAst) -> ComptimeValueDeclaration {
    ComptimeValueDeclaration(Rc::new(ComptimeValueDeclarationInner { ast }))
}

pub fn get_declaration(
    env: &mut Env,
    decl: ComptimeValueDeclaration,
) -> Result<ComptimeAnalysisResult, PositionedError> {
    let cache = env.decl_cache.clone();
    let key = decl.clone();
    cache.get_or_put(&key, env, move |env| {
        let scope = decl.ast().scope.clone();
        with_scope(env, scope, |env| {
            let mut block = empty_block();
            let result = analyze(
                env,
                Type::Unknown(TypeUnknown),
                decl.ast().pos.clone(),
                &decl.ast().ast,
                &mut block,
            )?;
            let evald =
                crate::comptime::comptime_eval(env, &block, result.value, decl.ast().pos.clone())?;
            Ok(ComptimeAnalysisResult {
                ty: result.ty,
                value: evald,
            })
        })
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockIdx(pub usize, pub Symbol);

#[derive(Debug, Clone)]
pub enum AnalysisLine {
    ComptimeKvListInit {
        pos: TokenPosition,
    },
    ComptimeKvListAppend {
        pos: TokenPosition,
        key: RuntimeValue,
        list: RuntimeValue,
        value: RuntimeValue,
    },
    Call {
        pos: TokenPosition,
        method: RuntimeValue,
        arg: RuntimeValue,
    },
    Break {
        pos: TokenPosition,
        value: RuntimeValue,
    },
    Args {
        pos: TokenPosition,
    },
    ComptimeFileCreate {
        pos: TokenPosition,
        value: RuntimeValue,
    },
    McExecRaw {
        pos: TokenPosition,
        command: RuntimeValue,
    },
}

#[derive(Debug, Clone)]
pub struct AnalysisBlock {
    pub offset: usize,
    pub lines: Vec<AnalysisLine>,
    pub validate: Symbol,
}

pub fn block_append(block: &mut AnalysisBlock, instr: AnalysisLine) -> BlockIdx {
    block.lines.push(instr);
    BlockIdx(block.lines.len() - 1, block.validate)
}

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub ty: Type,
    pub value: RuntimeValue,
}

#[derive(Debug, Clone)]
pub struct ComptimeAnalysisResult {
    pub ty: Type,
    pub value: ComptimeValue,
}

pub fn cast_value(to: Type, result: AnalysisResult) -> AnalysisResult {
    AnalysisResult {
        ty: to,
        value: result.value,
    }
}

#[derive(Debug, Clone)]
pub enum ComptimeValueKey {
    Symbol { key: Symbol, child: Type },
    String { key: String },
}

pub trait ComptimeNamespace: std::fmt::Debug {
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
        keychild: Type,
        field: Symbol,
        block: &mut AnalysisBlock,
    ) -> Result<Option<AnalysisResult>, PositionedError>;
    fn call(&self) -> Option<BuiltinFn>;
    fn pos(&self) -> &TokenPosition;
}

pub type BuiltinFn = for<'a> fn(
    &mut Env,
    Type,
    TokenPosition,
    CallArg<'a>,
    &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError>;

#[derive(Debug, Clone)]
pub struct ComptimeValueType {
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueVoid;

#[derive(Debug, Clone)]
pub struct NsFieldsEntry {
    pub pos: TokenPosition,
    pub key: ComptimeValue,
    pub value: ComptimeValue,
}

#[derive(Debug, Clone)]
pub struct NsFields {
    pub locked: bool,
    pub entries: Vec<NsFieldsEntry>,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueOptional {
    pub some: Option<Box<ComptimeValue>>,
}

#[derive(Debug, Clone)]
pub struct ComptimeFile {
    pub value: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ComptimeFolder {
    pub value: HashMap<String, ComptimeValueBuildArtifact>,
}

#[derive(Debug, Clone)]
pub enum ComptimeValueBuildArtifact {
    File(ComptimeFile),
    Folder(ComptimeFolder),
}

#[derive(Debug, Clone)]
pub struct Uint8ArraySourcemapEntry {
    pub len_bytes: usize,
    pub start_pos: TokenPosition,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueUint8Array {
    pub value: Vec<u8>,
    pub sourcemap: Vec<Uint8ArraySourcemapEntry>,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueCExportName {
    pub value: crate::backend::c::CValidatedIdentifierName,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueMcIdentifier {
    pub namespace: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueMcResult {
    pub result: i32,
}

#[derive(Debug, Clone)]
pub enum ComptimeValueMcNbtRef {
    String(String),
}

#[derive(Debug, Clone)]
pub struct ComptimeValueExportListEntry {
    pub key: ComptimeValue,
    pub key_pos: TokenPosition,
    pub value: ComptimeValueAst,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueExportList {
    pub exports: Vec<ComptimeValueExportListEntry>,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueError {
    pub etok: ConsumedErrorToken,
}

#[derive(Debug)]
struct ComptimeValueFnInner {
    args: Destructure,
    body: ComptimeValueAst,
    pos: TokenPosition,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueFn(Rc<ComptimeValueFnInner>);

impl ComptimeValueFn {
    pub fn new(args: Destructure, body: ComptimeValueAst, pos: TokenPosition) -> Self {
        ComptimeValueFn(Rc::new(ComptimeValueFnInner { args, body, pos }))
    }

    pub fn args(&self) -> &Destructure {
        &self.0.args
    }

    pub fn body(&self) -> &ComptimeValueAst {
        &self.0.body
    }

    pub fn pos(&self) -> &TokenPosition {
        &self.0.pos
    }
}

impl PartialEq for ComptimeValueFn {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ComptimeValueFn {}

impl std::hash::Hash for ComptimeValueFn {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (Rc::as_ptr(&self.0) as usize).hash(state);
    }
}

impl CacheKey for ComptimeValueFn {
    fn cache_ptr(&self) -> usize {
        Rc::as_ptr(&self.0) as usize
    }
}

#[derive(Debug, Clone)]
pub struct AnalyzedFn {
    pub block: AnalysisBlock,
    pub value: RuntimeValue,
}

#[derive(Debug, Clone)]
pub enum ComptimeValue {
    Key(ComptimeValueKey),
    Namespace(Rc<dyn ComptimeNamespace>),
    Type(ComptimeValueType),
    Ast(ComptimeValueAst),
    Void(ComptimeValueVoid),
    KvFields(NsFields),
    Fn(ComptimeValueFn),
    Optional(ComptimeValueOptional),
    BuildArtifact(ComptimeValueBuildArtifact),
    Uint8Array(ComptimeValueUint8Array),
    ExportList(ComptimeValueExportList),
    CExportName(ComptimeValueCExportName),
    McIdentifier(ComptimeValueMcIdentifier),
    McResult(ComptimeValueMcResult),
    McNbtRef(ComptimeValueMcNbtRef),
    Error(ComptimeValueError),
    Mc(crate::backend::mc::ComptimeValueMc),
}

#[derive(Debug, Clone)]
pub enum RuntimeValue {
    Comptime(ComptimeValue),
    Runtime(BlockIdx),
}

pub fn throw_err(
    env: &Env,
    pos: Option<TokenPosition>,
    msg: impl Into<String>,
    notes: Option<Vec<(Option<TokenPosition>, String)>>,
    style: Option<crate::parser::ErrorStyle>,
) -> PositionedError {
    PositionedError::Fresh(get_err(env, pos, msg, notes, style))
}

pub fn add_err(
    env: &mut Env,
    pos: Option<TokenPosition>,
    msg: impl Into<String>,
    notes: Option<Vec<(Option<TokenPosition>, String)>>,
) -> ConsumedErrorToken {
    let err = get_err(env, pos, msg, notes, None);
    env.errors.push(err);
    ConsumedErrorToken
}

pub fn get_err(
    env: &Env,
    pos: Option<TokenPosition>,
    msg: impl Into<String>,
    notes: Option<Vec<(Option<TokenPosition>, String)>>,
    style: Option<crate::parser::ErrorStyle>,
) -> TokenizationError {
    let mut entries = vec![TokenizationErrorEntry {
        pos,
        style: style.unwrap_or(crate::parser::ErrorStyle::Error),
        message: msg.into(),
    }];
    for (note_pos, note_msg) in notes.into_iter().flatten() {
        entries.push(TokenizationErrorEntry {
            pos: note_pos,
            style: crate::parser::ErrorStyle::Note,
            message: note_msg,
        });
    }
    TokenizationError {
        entries,
        trace: env.trace.clone(),
    }
}

fn handle_err(env: &mut Env, err: PositionedError) {
    if let PositionedError::Fresh(e) = err {
        env.errors.push(e);
    }
}

pub fn assert(condition: bool) {
    if !condition {
        panic!("assertion failed");
    }
}

pub(crate) fn syntax_node_kind(node: &SyntaxNode) -> &'static str {
    match node {
        SyntaxNode::Identifier(_) => "ident",
        SyntaxNode::Whitespace(_) => "ws",
        SyntaxNode::Operator(_) => "op",
        SyntaxNode::OperatorSegment(_) => "opSeg",
        SyntaxNode::Block(_) => "block",
        SyntaxNode::BinaryExpression(_) => "binary",
        SyntaxNode::Raw(_) => "raw",
        SyntaxNode::Err(_) => "err",
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
        SyntaxNode::Raw(t) => &t.pos,
        SyntaxNode::Err(t) => &t.pos,
    }
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

pub fn trim_ws(src: &[SyntaxNode]) -> Vec<SyntaxNode> {
    src.iter()
        .filter(|item| match item {
            SyntaxNode::Whitespace(_) => false,
            SyntaxNode::Block(b) if b.tag == BracketTag::InlineComment => false,
            _ => true,
        })
        .cloned()
        .collect()
}

pub type Binary2 = (OperatorSegmentToken, OperatorToken, OperatorSegmentToken);

pub fn read_binary2(
    env: &mut Env,
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
            Some(bin.pos.clone()),
            "Expected LHS op RHS, found not that",
            None,
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
                    None,
                ));
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct DestructureTarget {
    pub name: String,
    pub pos: TokenPosition,
}

#[derive(Debug, Clone)]
pub enum DestructureTag {
    CallConv {
        pos: TokenPosition,
    },
    Error {
        pos: TokenPosition,
        tok: ConsumedErrorToken,
    },
}

#[derive(Debug, Clone)]
pub struct Destructure {
    pub targets: Vec<DestructureTarget>,
    pub extract: DestructureExtract,
    pub ty: Type,
    pub tags: Vec<DestructureTag>,
}

#[derive(Debug, Clone)]
pub enum DestructureExtract {
    SingleItem {
        target: usize,
        pos: TokenPosition,
    },
    List {
        items: Vec<DestructureExtract>,
        pos: TokenPosition,
    },
    Map {
        items: Vec<(ComptimeValueKey, DestructureExtract)>,
        pos: TokenPosition,
    },
    Discard {
        pos: TokenPosition,
    },
}

fn destructure_extract_pos(extract: &DestructureExtract) -> &TokenPosition {
    match extract {
        DestructureExtract::SingleItem { pos, .. } => pos,
        DestructureExtract::List { pos, .. } => pos,
        DestructureExtract::Map { pos, .. } => pos,
        DestructureExtract::Discard { pos } => pos,
    }
}

pub fn read_destructure(
    env: &mut Env,
    pos: TokenPosition,
    src: &[SyntaxNode],
    targets: &mut Vec<DestructureTarget>,
) -> Result<Destructure, PositionedError> {
    let trimmed = trim_ws(src);
    let is_builtin_tag = |itm: &&SyntaxNode| matches!(itm, SyntaxNode::Identifier(id) if id.ident_tag == IdentifierTag::Builtin);
    let raw_tags: Vec<SyntaxNode> = trimmed.iter().filter(is_builtin_tag).cloned().collect();
    let lhs_items: Vec<SyntaxNode> = trimmed
        .iter()
        .filter(|itm| !is_builtin_tag(itm))
        .cloned()
        .collect();
    if lhs_items.is_empty() {
        return Err(throw_err(
            env,
            Some(pos),
            format!(
                "Expected at least one item to destructure{}",
                AST_NODE.dump_list(src, 2)
            ),
            None,
            None,
        ));
    }

    let mut ty: Option<Type> = None;
    {
        let last = lhs_items.last().expect("checked non-empty above");
        if let SyntaxNode::Block(b) = last {
            if b.tag == BracketTag::ColonCall {
                let mut sub_block = empty_block();
                let body = analyze(
                    env,
                    Type::CtType(CtType),
                    b.pos.clone(),
                    &b.items,
                    &mut sub_block,
                )?;
                let evaluated =
                    crate::comptime::comptime_eval(env, &sub_block, body.value, b.pos.clone())?;
                let got = crate::comptime::get_comptime(
                    env,
                    Some(crate::comptime::ComptimeValueKind::Type),
                    RuntimeValue::Comptime(evaluated),
                    b.pos.clone(),
                )?;
                let ComptimeValue::Type(got) = got else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                ty = Some(got.ty);
            }
        }
    }

    if lhs_items.len() > 1 {
        return Err(throw_err(
            env,
            Some(syntax_node_pos(&lhs_items[1]).clone()),
            format!(
                "Unexpected item for destructuring. TODO support eg 'name: type := value'{}",
                AST_NODE.dump_list(src, 2)
            ),
            None,
            None,
        ));
    }

    let _processed_tags: Vec<DestructureTag> = raw_tags
        .into_iter()
        .map(|tag| {
            if let SyntaxNode::Identifier(id) = &tag {
                if id.str == "callconv_c" {
                    return DestructureTag::CallConv {
                        pos: id.pos.clone(),
                    };
                }
            }
            let tag_pos = syntax_node_pos(&tag).clone();
            let tok = add_err(
                env,
                Some(tag_pos.clone()),
                format!("Bad tag: {}", AST_NODE.dump(&tag, 3)),
                None,
            );
            DestructureTag::Error { pos: tag_pos, tok }
        })
        .collect();

    let ident = &lhs_items[0];
    match ident {
        SyntaxNode::Identifier(id) if id.ident_tag == IdentifierTag::Normal => {
            let target_idx = targets.len();
            targets.push(DestructureTarget {
                name: id.str.clone(),
                pos: id.pos.clone(),
            });
            Ok(Destructure {
                extract: DestructureExtract::SingleItem {
                    target: target_idx,
                    pos: id.pos.clone(),
                },
                ty: ty.unwrap_or(Type::Unknown(TypeUnknown)),
                tags: Vec::new(),
                targets: targets.clone(),
            })
        }
        SyntaxNode::Identifier(id) if id.ident_tag == IdentifierTag::Discard => Ok(Destructure {
            extract: DestructureExtract::Discard {
                pos: id.pos.clone(),
            },
            ty: Type::Unknown(TypeUnknown),
            tags: Vec::new(),
            targets: targets.clone(),
        }),
        SyntaxNode::Block(b) if b.tag == BracketTag::List => {
            let args = read_binary(env, b.pos.clone(), &b.items, OpTag::Sep)?;
            let mut extracts = Vec::new();
            let mut types = Vec::new();
            for arg in &args {
                if arg.items.is_empty() {
                    continue;
                }
                let sub = read_destructure(env, arg.pos.clone(), &arg.items, targets)?;
                extracts.push(sub.extract);
                types.push(sub.ty);
            }
            if ty.is_some() {
                return Err(throw_err(
                    env,
                    Some(b.pos.clone()),
                    "TODO support setting type on block in destructure",
                    None,
                    None,
                ));
            }
            Ok(Destructure {
                extract: DestructureExtract::List {
                    items: extracts,
                    pos: b.pos.clone(),
                },
                ty: Type::Tuple(TypeTuple { children: types }),
                tags: Vec::new(),
                targets: targets.clone(),
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
            None,
        )),
    }
}

fn analyze_destructure(
    env: &mut Env,
    destructure: &Destructure,
    body: AnalysisResult,
    block: &mut AnalysisBlock,
) -> Result<Vec<AnalysisResult>, PositionedError> {
    let mut targets: Vec<Option<AnalysisResult>> =
        (0..destructure.targets.len()).map(|_| None).collect();
    analyze_destructure_inner(env, &destructure.extract, body, block, &mut targets)?;
    assert!(targets.iter().all(|t| t.is_some()));
    Ok(targets
        .into_iter()
        .map(|t| t.expect("checked all Some above"))
        .collect())
}

fn analyze_destructure_inner(
    env: &mut Env,
    extract: &DestructureExtract,
    body: AnalysisResult,
    _block: &mut AnalysisBlock,
    targets: &mut [Option<AnalysisResult>],
) -> Result<(), PositionedError> {
    match extract {
        DestructureExtract::SingleItem { target, .. } => {
            assert!(targets[*target].is_none());
            targets[*target] = Some(body);
            Ok(())
        }
        DestructureExtract::Discard { .. } => Ok(()),
        _ => Err(throw_err(
            env,
            Some(destructure_extract_pos(extract).clone()),
            format!(
                "TODO support extract: {}",
                crate::printers::printers::DESTRUCTURE_EXTRACT.dump(extract, 3)
            ),
            None,
            None,
        )),
    }
}

pub struct ReadContainer {
    pub lines: Vec<OperatorSegmentToken>,
}

fn read_container_line(
    env: &mut Env,
    res: &mut ReadContainer,
    line_raw: &OperatorSegmentToken,
) -> Result<(), PositionedError> {
    let line_items = trim_ws(&line_raw.items);
    if line_items.is_empty() {
        return Ok(());
    }
    if let Some((lhs, op, rhs)) = read_binary2(env, &line_items, OpTag::Def)? {
        let mut targets = Vec::new();
        let destructure = read_destructure(env, lhs.pos.clone(), &lhs.items, &mut targets)?;
        if !matches!(destructure.extract, DestructureExtract::SingleItem { .. }) {
            return Err(throw_err(
                env,
                Some(destructure_extract_pos(&destructure.extract).clone()),
                "TODO: support multiple destructure targets using analyze_destructure(), called when any of the names is resolved (but if two multiple names are resolved, called only once)",
                None,
                None,
            ));
        }
        for target in &destructure.targets {
            if let Some(prev) = env.scope.bindings.get(&target.name) {
                let prev_pos = binding_pos(prev).clone();
                let tok = add_err(
                    env,
                    Some(destructure_extract_pos(&destructure.extract).clone()),
                    format!("Duplicate binding name {}", target.name),
                    Some(vec![(
                        Some(prev_pos.clone()),
                        "Previous definition here".to_string(),
                    )]),
                );
                env.scope.bindings.insert(
                    target.name.clone(),
                    Binding::Error {
                        pos: prev_pos,
                        consumed: tok,
                    },
                );
            } else {
                let scope = env.scope.clone();
                let decl = create_declaration(
                    env,
                    ComptimeValueAst {
                        ast: rhs.items.clone(),
                        pos: rhs.pos.clone(),
                        scope,
                    },
                );
                env.scope.bindings.insert(
                    target.name.clone(),
                    Binding::Valid {
                        pos: op.pos.clone(),
                        decl,
                    },
                );
            }
        }
    } else {
        res.lines.push(line_raw.clone());
    }
    Ok(())
}

pub fn read_container(
    env: &mut Env,
    pos: TokenPosition,
    src: &[SyntaxNode],
) -> Result<ReadContainer, PositionedError> {
    let lines = read_binary(env, pos, src, OpTag::Sep)?;
    let mut res = ReadContainer { lines: Vec::new() };
    for line in &lines {
        if let Err(e) = read_container_line(env, &mut res, line) {
            handle_err(env, e);
        }
    }
    Ok(res)
}

pub fn analyze_block(
    env: &mut Env,
    slot: Type,
    pos: TokenPosition,
    src: &[SyntaxNode],
    block: &mut AnalysisBlock,
    mut analyze_bind: impl FnMut(
        &mut Env,
        Binary2,
        &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError>,
) -> Result<AnalysisResult, PositionedError> {
    let saved_bindings = env.scope.bindings.clone();
    let result = analyze_block_body(env, slot, pos, src, block, &mut analyze_bind);
    env.scope.bindings = saved_bindings;
    result
}

fn analyze_block_body(
    env: &mut Env,
    slot: Type,
    pos: TokenPosition,
    src: &[SyntaxNode],
    block: &mut AnalysisBlock,
    analyze_bind: &mut dyn FnMut(
        &mut Env,
        Binary2,
        &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError>,
) -> Result<AnalysisResult, PositionedError> {
    let container = read_container(env, pos, src)?;

    let mut ret: Option<AnalysisResult> = None;
    let mut retloc: Option<TokenPosition> = None;
    for line in &container.lines {
        if ret.is_some() {
            add_err(
                env,
                Some(line.pos.clone()),
                "extra lines not allowed after return",
                retloc
                    .clone()
                    .map(|p| vec![(Some(p), "returned here".to_string())]),
            );
            break;
        }
        let rb2 = read_binary2(env, &line.items, OpTag::Pub)?;
        if let Some(b2) = rb2 {
            analyze_bind(env, b2, block)?;
        } else {
            let rb3 = read_binary2(env, &line.items, OpTag::Var)?;
            if let Some((lhs, _op, rhs)) = rb3 {
                let mut targets = Vec::new();
                let destructure = read_destructure(env, lhs.pos.clone(), &lhs.items, &mut targets)?;
                let rhs_analyzed = analyze(
                    env,
                    destructure.ty.clone(),
                    rhs.pos.clone(),
                    &rhs.items,
                    block,
                )?;
                let destructured = analyze_destructure(env, &destructure, rhs_analyzed, block)?;
                for (target, value) in destructure.targets.iter().zip(destructured) {
                    env.scope.bindings.insert(
                        target.name.clone(),
                        Binding::Runtime {
                            pos: target.pos.clone(),
                            runtime: value,
                        },
                    );
                }
            } else {
                let trimmed = trim_ws(&line.items);
                if let Some(SyntaxNode::Raw(r)) = trimmed.first() {
                    if r.tag == RawTag::Return {
                        retloc = Some(r.pos.clone());
                        ret = Some(analyze(
                            env,
                            slot.clone(),
                            line.pos.clone(),
                            &trimmed[1..],
                            block,
                        )?);
                        continue;
                    }
                }
                analyze(
                    env,
                    Type::Void(TypeVoid),
                    line.pos.clone(),
                    &line.items,
                    block,
                )?;
            }
        }
    }

    if let Some(ret) = ret {
        return Ok(ret);
    }
    Ok(AnalysisResult {
        ty: Type::Void(TypeVoid),
        value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
    })
}

#[derive(Debug)]
enum RegisteredEntry {
    Ok {
        decl: ComptimeValueDeclaration,
        pos: TokenPosition,
    },
    Error {
        etok: ConsumedErrorToken,
        pos: TokenPosition,
    },
}

#[derive(Debug)]
struct NamespaceImpl {
    pos: TokenPosition,
    registered: HashMap<NsKey, RegisteredEntry>,
}

impl ComptimeNamespace for NamespaceImpl {
    fn get_string(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        field: &str,
        _block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        if self.registered.contains_key(&NsKey::Str(field.to_string())) {
            return Err(throw_err(
                env,
                Some(pos),
                "todo get registered field",
                None,
                None,
            ));
        }
        Err(throw_err(
            env,
            Some(pos.clone()),
            format!("string field '{field}' is not defined on namespace"),
            Some(vec![(Some(pos), "namespace declared here".to_string())]),
            None,
        ))
    }

    fn get_symbol(
        &self,
        env: &mut Env,
        _pos: TokenPosition,
        _keychild: Type,
        field: Symbol,
        _block: &mut AnalysisBlock,
    ) -> Result<Option<AnalysisResult>, PositionedError> {
        let Some(entry) = self.registered.get(&NsKey::Sym(field)) else {
            return Ok(None);
        };
        match entry {
            RegisteredEntry::Error { etok, .. } => Err(throw_consumed_err(*etok)),
            RegisteredEntry::Ok { decl, .. } => {
                let r = get_declaration(env, decl.clone())?;
                Ok(Some(AnalysisResult {
                    ty: r.ty,
                    value: RuntimeValue::Comptime(r.value),
                }))
            }
        }
    }

    fn call(&self) -> Option<BuiltinFn> {
        None
    }

    fn pos(&self) -> &TokenPosition {
        &self.pos
    }
}

pub fn analyze_namespace(
    env: &mut Env,
    pos: TokenPosition,
    src: &[SyntaxNode],
) -> Result<Rc<dyn ComptimeNamespace>, PositionedError> {
    let mut block = empty_block();
    let arr_entry = block_append(
        &mut block,
        AnalysisLine::ComptimeKvListInit { pos: pos.clone() },
    );

    analyze_block(
        env,
        Type::Void(TypeVoid),
        pos.clone(),
        src,
        &mut block,
        |env, b2, block| {
            let (lhs, op, rhs) = b2;
            let key = analyze(env, Type::CtKey(CtKey), lhs.pos.clone(), &lhs.items, block)?;
            let kval = crate::comptime::get_comptime(
                env,
                Some(crate::comptime::ComptimeValueKind::Key),
                key.value,
                lhs.pos.clone(),
            )?;
            let value = analyze(env, Type::CtAst(CtAst), rhs.pos.clone(), &rhs.items, block)?;
            let ret = block_append(
                block,
                AnalysisLine::ComptimeKvListAppend {
                    pos: op.pos.clone(),
                    list: RuntimeValue::Runtime(arr_entry),
                    key: RuntimeValue::Comptime(kval),
                    value: value.value,
                },
            );
            Ok(AnalysisResult {
                ty: Type::Void(TypeVoid),
                value: RuntimeValue::Runtime(ret),
            })
        },
    )?;

    let evaluated =
        crate::comptime::comptime_eval(env, &block, RuntimeValue::Runtime(arr_entry), pos.clone())?;
    let arr_value = crate::comptime::get_comptime(
        env,
        Some(crate::comptime::ComptimeValueKind::KvFields),
        RuntimeValue::Comptime(evaluated),
        pos.clone(),
    )?;
    let ComptimeValue::KvFields(mut arr_value) = arr_value else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    arr_value.locked = true;

    let mut registered: HashMap<NsKey, RegisteredEntry> = HashMap::new();
    for entry in arr_value.entries {
        let key = crate::comptime::get_comptime(
            env,
            Some(crate::comptime::ComptimeValueKind::Key),
            RuntimeValue::Comptime(entry.key),
            entry.pos.clone(),
        )?;
        let ComptimeValue::Key(key_val) = key else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        let value = crate::comptime::get_comptime(
            env,
            Some(crate::comptime::ComptimeValueKind::Ast),
            RuntimeValue::Comptime(entry.value),
            entry.pos.clone(),
        )?;
        let ComptimeValue::Ast(value_val) = value else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        let ns_key = match &key_val {
            ComptimeValueKey::String { key } => NsKey::Str(key.clone()),
            ComptimeValueKey::Symbol { key, .. } => NsKey::Sym(*key),
        };
        if let Some(prev) = registered.get(&ns_key) {
            let prev_pos = match prev {
                RegisteredEntry::Ok { pos, .. } => pos.clone(),
                RegisteredEntry::Error { pos, .. } => pos.clone(),
            };
            let etok = add_err(
                env,
                Some(entry.pos.clone()),
                "duplicate definition",
                Some(vec![(
                    Some(prev_pos.clone()),
                    "previous definition here".to_string(),
                )]),
            );
            registered.insert(
                ns_key,
                RegisteredEntry::Error {
                    etok,
                    pos: prev_pos,
                },
            );
            continue;
        }
        registered.insert(
            ns_key,
            RegisteredEntry::Ok {
                decl: create_declaration(env, value_val),
                pos: entry.pos.clone(),
            },
        );
    }

    Ok(Rc::new(NamespaceImpl { pos, registered }))
}

pub fn analyze_function(
    env: &mut Env,
    fn_value: &ComptimeValueFn,
) -> Result<AnalyzedFn, PositionedError> {
    let saved_bindings = std::mem::replace(
        &mut env.scope.bindings,
        fn_value.body().scope.bindings.clone(),
    );
    let cache = env.fn_cache.clone();
    let result = cache.get_or_put(fn_value, env, |env| {
        let mut block = empty_block();
        block_append(
            &mut block,
            AnalysisLine::Args {
                pos: destructure_extract_pos(&fn_value.args().extract).clone(),
            },
        );
        let result = analyze(
            env,
            Type::Unknown(TypeUnknown),
            fn_value.pos().clone(),
            &fn_value.body().ast,
            &mut block,
        )?;
        Ok(AnalyzedFn {
            block,
            value: result.value,
        })
    });
    env.scope.bindings = saved_bindings;
    result
}

pub fn cast_or_analyze() {}

pub fn analyze(
    env: &mut Env,
    slot: Type,
    pos: TokenPosition,
    ast: &[SyntaxNode],
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    if let Type::CtAst(_) = &slot {
        let value = ComptimeValueAst {
            ast: ast.to_vec(),
            pos,
            scope: env.scope.clone(),
        };
        return Ok(AnalysisResult {
            ty: Type::CtAst(CtAst),
            value: RuntimeValue::Comptime(ComptimeValue::Ast(value)),
        });
    }
    let ast = trim_ws(ast);

    if ast.is_empty() {
        return Err(throw_err(
            env,
            Some(pos),
            "failed to analyze empty expression",
            None,
            None,
        ));
    }
    if let SyntaxNode::Raw(r) = &ast[0] {
        if r.tag == RawTag::Return {
            return Err(throw_err(
                env,
                Some(r.pos.clone()),
                "can't return here",
                None,
                None,
            ));
        }
    }

    let last = ast.len() - 1;
    analyze_sub(env, slot.clone(), slot, &ast, last, block)
}

pub fn analyze_sub(
    env: &mut Env,
    slot: Type,
    root_slot: Type,
    ast: &[SyntaxNode],
    index: usize,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let expr = &ast[index];

    if let SyntaxNode::Identifier(id) = expr {
        if id.ident_tag == IdentifierTag::Access {
            let lhs = if index >= 1 {
                analyze_sub(
                    env,
                    Type::Unknown(TypeUnknown),
                    root_slot.clone(),
                    ast,
                    index - 1,
                    block,
                )?
            } else {
                AnalysisResult {
                    ty: Type::CtType(CtType),
                    value: RuntimeValue::Comptime(ComptimeValue::Type(ComptimeValueType {
                        ty: root_slot.clone(),
                    })),
                }
            };
            let prop = AnalysisResult {
                ty: Type::CtKey(CtKey),
                value: RuntimeValue::Comptime(ComptimeValue::Key(ComptimeValueKey::String {
                    key: id.str.clone(),
                })),
            };
            return analyze_access(env, slot, lhs, id.pos.clone(), prop, block);
        }
    } else if let SyntaxNode::Block(b) = expr {
        if b.tag == BracketTag::ArrowFn {
            let sts = slot.implicit_arg_ret_for_arrow_fn();
            let mut targets = Vec::new();
            let args = read_destructure(env, b.pos.clone(), &ast[..index], &mut targets)?;
            let ret_ty = Type::Fn(TypeFn {
                pos: b.pos.clone(),
                arg: Box::new(args.ty.clone()),
                ret: Box::new(sts.ret),
            });
            let fn_value = ComptimeValueFn::new(
                args,
                ComptimeValueAst {
                    ast: b.items.clone(),
                    pos: b.pos.clone(),
                    scope: env.scope.clone(),
                },
                b.pos.clone(),
            );
            return Ok(AnalysisResult {
                ty: ret_ty,
                value: RuntimeValue::Comptime(ComptimeValue::Fn(fn_value)),
            });
        } else if b.tag == BracketTag::ColonCall {
            let lhs = analyze_sub(
                env,
                Type::Unknown(TypeUnknown),
                root_slot,
                ast,
                index - 1,
                block,
            )?;
            return analyze_call(
                env,
                slot,
                b.pos.clone(),
                lhs,
                CallArg {
                    pos: b.pos.clone(),
                    ast: &b.items,
                },
                block,
            );
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
                AST_NODE.dump_list(std::slice::from_ref(expr), 2)
            ),
            None,
            None,
        ))
    }
}

pub fn analyze_call(
    env: &mut Env,
    slot: Type,
    pos: TokenPosition,
    method: AnalysisResult,
    arg_in: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let ty = method.ty.clone();
    ty.analyze_call(env, slot, pos, method, arg_in, block)
}

pub fn analyze_access(
    env: &mut Env,
    slot: Type,
    obj: AnalysisResult,
    pos: TokenPosition,
    prop: AnalysisResult,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let ty = obj.ty.clone();
    ty.analyze_access(env, slot, obj, pos, prop, block)
}

pub fn analyze_base(
    env: &mut Env,
    slot: Type,
    ast: &SyntaxNode,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    match ast {
        SyntaxNode::Identifier(id) if id.ident_tag == IdentifierTag::Builtin => {
            if id.str == "builtin" {
                descriptor_construct(&builtin_namespace_descriptor(), env, "#builtin")
            } else {
                Err(throw_err(
                    env,
                    Some(id.pos.clone()),
                    format!("unexpected builtin: #{}", id.str),
                    None,
                    None,
                ))
            }
        }
        SyntaxNode::Identifier(id) if id.ident_tag == IdentifierTag::Normal => {
            let Some(value) = env.scope.bindings.get(&id.str).cloned() else {
                return Err(throw_err(
                    env,
                    Some(id.pos.clone()),
                    format!("not defined in scope: {}", id.str),
                    None,
                    None,
                ));
            };
            match value {
                Binding::Error { consumed, .. } => Err(throw_consumed_err(consumed)),
                Binding::Removed { pos } => Err(throw_err(
                    env,
                    Some(id.pos.clone()),
                    format!("not defined in scope: {}", id.str),
                    Some(vec![(Some(pos), "removed here".to_string())]),
                    None,
                )),
                Binding::Valid { decl, .. } => {
                    let r = get_declaration(env, decl)?;
                    Ok(AnalysisResult {
                        ty: r.ty,
                        value: RuntimeValue::Comptime(r.value),
                    })
                }
                Binding::Runtime { runtime, .. } => {
                    if let RuntimeValue::Runtime(idx) = &runtime.value {
                        if idx.1 != block.validate {
                            return Err(throw_err(
                                env,
                                Some(id.pos.clone()),
                                "not accessible",
                                None,
                                None,
                            ));
                        }
                    }
                    Ok(runtime)
                }
            }
        }
        SyntaxNode::Block(b) if b.tag == BracketTag::String => {
            slot.clone().from_string(env, slot, b, block)
        }
        SyntaxNode::Raw(r) if r.tag == RawTag::Void => Ok(AnalysisResult {
            ty: Type::Void(TypeVoid),
            value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
        }),
        SyntaxNode::Block(b) if b.tag == BracketTag::Map => {
            slot.clone().from_map(env, slot, b, block)
        }
        SyntaxNode::Block(b) if b.tag == BracketTag::Code => analyze_block(
            env,
            slot,
            b.pos.clone(),
            &b.items,
            block,
            |env, b2, _block| {
                let (_lhs, op, _rhs) = b2;
                Err(throw_err(
                    env,
                    Some(op.pos.clone()),
                    "TODO: implement bind in block",
                    None,
                    None,
                ))
            },
        ),
        SyntaxNode::Identifier(id) if id.ident_tag == IdentifierTag::Number => {
            slot.clone().from_number(env, slot, id, block)
        }
        SyntaxNode::BinaryExpression(be) if be.tag == OpTag::Assign => {
            let rbr = read_binary2(env, std::slice::from_ref(ast), OpTag::Assign)?;
            let Some((lhs, _op, rhs)) = rbr else {
                return Err(throw_err(
                    env,
                    Some(be.pos.clone()),
                    format!(
                        "Expected X = Y, got X = Y = Z? {}",
                        AST_NODE.dump_list(&be.items, crate::printers::UNLIMITED_DEPTH)
                    ),
                    None,
                    None,
                ));
            };
            let mut targets = Vec::new();
            let destructure = read_destructure(env, lhs.pos.clone(), &lhs.items, &mut targets)?;
            let body = analyze(
                env,
                destructure.ty.clone(),
                rhs.pos.clone(),
                &rhs.items,
                block,
            )?;
            let bindings = analyze_destructure(env, &destructure, body, block)?;
            if !bindings.is_empty() {
                return Err(throw_err(
                    env,
                    Some(lhs.pos.clone()),
                    "TODO implement assignment operator",
                    None,
                    None,
                ));
            }
            Ok(AnalysisResult {
                ty: Type::Void(TypeVoid),
                value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
            })
        }
        _ => Err(throw_err(
            env,
            Some(syntax_node_pos(ast).clone()),
            format!(
                "TODO analyzeBase: {}{}",
                syntax_node_kind(ast),
                AST_NODE.dump_list(std::slice::from_ref(ast), 3)
            ),
            None,
            None,
        )),
    }
}

pub trait Descriptor: std::fmt::Debug {
    fn construct_impl(&self, env: &mut Env, route: &str)
        -> Result<AnalysisResult, PositionedError>;
}

fn descriptor_construct(
    d: &Rc<dyn Descriptor>,
    env: &mut Env,
    route: &str,
) -> Result<AnalysisResult, PositionedError> {
    let cache = env.builtin_cache.clone();
    let d2 = d.clone();
    let route2 = route.to_string();
    cache.get_or_put(d, env, move |env| d2.construct_impl(env, &route2))
}

impl CacheKey for Rc<dyn Descriptor> {
    fn cache_ptr(&self) -> usize {
        Rc::as_ptr(self) as *const () as usize
    }
}

#[derive(Debug)]
pub struct NamespaceDescriptor {
    pub entries: Vec<(String, Rc<dyn Descriptor>)>,
    pub call: Option<BuiltinFn>,
    pub pos: TokenPosition,
}

impl Descriptor for NamespaceDescriptor {
    fn construct_impl(
        &self,
        env: &mut Env,
        route: &str,
    ) -> Result<AnalysisResult, PositionedError> {
        let mut results: HashMap<String, AnalysisResult> = HashMap::new();
        for (key, value) in &self.entries {
            let sub_route = format!("{route}.{key}");
            results.insert(key.clone(), descriptor_construct(value, env, &sub_route)?);
        }
        let ns = BuiltinNamespaceImpl {
            results,
            call: self.call,
            pos: self.pos.clone(),
            route: route.to_string(),
        };
        Ok(AnalysisResult {
            ty: Type::CtNamespace(CtNamespace),
            value: RuntimeValue::Comptime(ComptimeValue::Namespace(Rc::new(ns))),
        })
    }
}

#[derive(Debug)]
struct BuiltinNamespaceImpl {
    results: HashMap<String, AnalysisResult>,
    call: Option<BuiltinFn>,
    pos: TokenPosition,
    route: String,
}

impl ComptimeNamespace for BuiltinNamespaceImpl {
    fn get_string(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        field: &str,
        _block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        if let Some(v) = self.results.get(field) {
            return Ok(v.clone());
        }
        Err(throw_err(
            env,
            Some(pos),
            format!("namespace {} does not have field: {field}", self.route),
            Some(vec![(
                Some(self.pos.clone()),
                "namespace defined here".to_string(),
            )]),
            None,
        ))
    }

    fn get_symbol(
        &self,
        _env: &mut Env,
        _pos: TokenPosition,
        _keychild: Type,
        _field: Symbol,
        _block: &mut AnalysisBlock,
    ) -> Result<Option<AnalysisResult>, PositionedError> {
        Ok(None)
    }

    fn call(&self) -> Option<BuiltinFn> {
        self.call
    }

    fn pos(&self) -> &TokenPosition {
        &self.pos
    }
}

#[derive(Debug)]
pub struct CustomDescriptor(pub AnalysisResult);

impl Descriptor for CustomDescriptor {
    fn construct_impl(
        &self,
        _env: &mut Env,
        _route: &str,
    ) -> Result<AnalysisResult, PositionedError> {
        Ok(self.0.clone())
    }
}

fn d_raw(result: AnalysisResult) -> Rc<dyn Descriptor> {
    Rc::new(CustomDescriptor(result))
}

fn d_ns(entries: Vec<(&str, Rc<dyn Descriptor>)>, call: Option<BuiltinFn>) -> Rc<dyn Descriptor> {
    Rc::new(NamespaceDescriptor {
        entries: entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        call,
        pos: compiler_pos(),
    })
}

fn std_folder_or_file_type() -> Type {
    Type::CtBuildArtifact(CtBuildArtifact { narrow: None })
}

fn build_symbol() -> Symbol {
    static BUILD_SYMBOL: OnceLock<Symbol> = OnceLock::new();
    *BUILD_SYMBOL.get_or_init(Symbol::new)
}

fn build_symbol_child_type() -> Type {
    Type::Fn(TypeFn {
        pos: compiler_pos(),
        arg: Box::new(Type::Void(TypeVoid)),
        ret: Box::new(std_folder_or_file_type()),
    })
}

fn build_symbol_value() -> ComptimeValueKey {
    ComptimeValueKey::Symbol {
        key: build_symbol(),
        child: build_symbol_child_type(),
    }
}

fn builtin_mc_run_command_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let arg = analyze(
        env,
        Type::McNbtRef(McNbtRef {
            narrow: Some(McNbtRefNarrow::String),
        }),
        arg_ast.pos,
        arg_ast.ast,
        block,
    )?;
    let res = block_append(
        block,
        AnalysisLine::McExecRaw {
            pos,
            command: arg.value,
        },
    );
    Ok(AnalysisResult {
        ty: Type::McResult(McResult { narrow: None }),
        value: RuntimeValue::Runtime(res),
    })
}

fn builtin_mc_datapack_compile_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    with_target_env(env, TargetEnv::Mc, |env| {
        let arg_res = analyze(
            env,
            Type::CtExportList(CtExportList {
                key: Box::new(Type::McIdentifier(McIdentifier { category: None })),
            }),
            arg_ast.pos,
            arg_ast.ast,
            block,
        )?;
        let arg_ct = crate::comptime::get_comptime(
            env,
            Some(crate::comptime::ComptimeValueKind::ExportList),
            arg_res.value,
            pos.clone(),
        )?;
        let ComptimeValue::ExportList(arg_ct) = arg_ct else {
            unreachable!("get_comptime guarantees a matching kind")
        };

        let mut ctx = crate::backend::mc::McCodegenCtx {
            fns: HashMap::new(),
            gid: 0,
            internal_ns: "_0".to_string(),
        };

        for item in &arg_ct.exports {
            let ident = crate::comptime::get_comptime(
                env,
                Some(crate::comptime::ComptimeValueKind::McIdentifier),
                RuntimeValue::Comptime(item.key.clone()),
                pos.clone(),
            )?;
            let ComptimeValue::McIdentifier(ident) = ident else {
                unreachable!("get_comptime guarantees a matching kind")
            };
            let body = analyze(
                env,
                Type::Unknown(TypeUnknown),
                item.value.pos.clone(),
                &item.value.ast,
                block,
            )?;
            if let Type::Fn(_) = &body.ty {
                let content = crate::comptime::get_comptime(
                    env,
                    Some(crate::comptime::ComptimeValueKind::Fn),
                    body.value,
                    item.value.pos.clone(),
                )?;
                let ComptimeValue::Fn(content) = content else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                if ctx.fns.contains_key(&content) {
                    return Err(throw_err(
                        env,
                        Some(pos.clone()),
                        "duplicate item",
                        None,
                        None,
                    ));
                }
                ctx.fns.insert(content, ident);
            } else {
                return Err(throw_err(
                    env,
                    Some(pos.clone()),
                    format!(
                        "TODO mc body type: {}",
                        crate::printers::printers::RUNTIME_VALUE.dump_list(
                            &[
                                RuntimeValue::Comptime(item.key.clone()),
                                RuntimeValue::Comptime(ComptimeValue::Ast(item.value.clone())),
                            ],
                            crate::printers::UNLIMITED_DEPTH,
                        )
                    ),
                    None,
                    None,
                ));
            }
        }

        let mut res_files: HashMap<String, Vec<u8>> = HashMap::new();
        for (content, ident) in ctx.fns.clone() {
            let compiled = analyze_function(env, &content)?;
            let result = crate::backend::mc::codegen_mcfunction(
                env,
                &mut ctx,
                &compiled.block,
                compiled.value,
            )?;
            res_files.insert(
                format!(
                    "data/{}/functions/{}.mcfunction",
                    ident.namespace, ident.path
                ),
                result.into_bytes(),
            );
        }

        Ok(AnalysisResult {
            ty: Type::CtBuildArtifact(CtBuildArtifact {
                narrow: Some(CtBuildArtifactNarrow::Folder),
            }),
            value: RuntimeValue::Comptime(ComptimeValue::BuildArtifact(
                ComptimeValueBuildArtifact::Folder(ComptimeFolder {
                    value: res_files
                        .into_iter()
                        .map(|(k, v)| {
                            (
                                k,
                                ComptimeValueBuildArtifact::File(ComptimeFile { value: v }),
                            )
                        })
                        .collect(),
                }),
            )),
        })
    })
}

fn builtin_c_compile_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    with_target_env(env, TargetEnv::C, |env| {
        let arg_res = analyze(
            env,
            Type::CtExportList(CtExportList {
                key: Box::new(Type::CExportName(CExportName)),
            }),
            arg_ast.pos,
            arg_ast.ast,
            block,
        )?;
        let arg_ct = crate::comptime::get_comptime(
            env,
            Some(crate::comptime::ComptimeValueKind::ExportList),
            arg_res.value,
            pos.clone(),
        )?;
        let ComptimeValue::ExportList(arg_ct) = arg_ct else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        for entry in &arg_ct.exports {
            let _kv = crate::comptime::get_comptime(
                env,
                Some(crate::comptime::ComptimeValueKind::CExportName),
                RuntimeValue::Comptime(entry.key.clone()),
                entry.key_pos.clone(),
            )?;
        }
        Err(throw_err(
            env,
            Some(pos),
            "TODO call #builtin.std.c.compile",
            None,
            None,
        ))
    })
}

fn build_builtin_namespace_descriptor() -> Rc<dyn Descriptor> {
    d_ns(
        vec![
            (
                "build",
                d_raw(AnalysisResult {
                    ty: Type::CtKey(CtKey),
                    value: RuntimeValue::Comptime(ComptimeValue::Key(build_symbol_value())),
                }),
            ),
            (
                "std",
                d_ns(
                    vec![
                        (
                            "File",
                            d_raw(AnalysisResult {
                                ty: Type::CtType(CtType),
                                value: RuntimeValue::Comptime(ComptimeValue::Type(
                                    ComptimeValueType {
                                        ty: Type::CtBuildArtifact(CtBuildArtifact {
                                            narrow: Some(CtBuildArtifactNarrow::File),
                                        }),
                                    },
                                )),
                            }),
                        ),
                        (
                            "Folder",
                            d_raw(AnalysisResult {
                                ty: Type::CtType(CtType),
                                value: RuntimeValue::Comptime(ComptimeValue::Type(
                                    ComptimeValueType {
                                        ty: Type::CtBuildArtifact(CtBuildArtifact {
                                            narrow: Some(CtBuildArtifactNarrow::Folder),
                                        }),
                                    },
                                )),
                            }),
                        ),
                        (
                            "mc",
                            d_ns(
                                vec![
                                    (
                                        "runCommand",
                                        d_ns(vec![], Some(builtin_mc_run_command_call)),
                                    ),
                                    (
                                        "Result",
                                        d_raw(AnalysisResult {
                                            ty: Type::CtType(CtType),
                                            value: RuntimeValue::Comptime(ComptimeValue::Type(
                                                ComptimeValueType {
                                                    ty: Type::McResult(McResult { narrow: None }),
                                                },
                                            )),
                                        }),
                                    ),
                                    (
                                        "Datapack",
                                        d_ns(
                                            vec![(
                                                "compile",
                                                d_ns(
                                                    vec![],
                                                    Some(builtin_mc_datapack_compile_call),
                                                ),
                                            )],
                                            None,
                                        ),
                                    ),
                                ],
                                None,
                            ),
                        ),
                        (
                            "c",
                            d_ns(
                                vec![("compile", d_ns(vec![], Some(builtin_c_compile_call)))],
                                None,
                            ),
                        ),
                    ],
                    None,
                ),
            ),
        ],
        None,
    )
}

thread_local! {
    static BUILTIN_NAMESPACE: Rc<dyn Descriptor> = build_builtin_namespace_descriptor();
}

fn builtin_namespace_descriptor() -> Rc<dyn Descriptor> {
    BUILTIN_NAMESPACE.with(|d| d.clone())
}

fn import_file_body(
    env: &mut Env,
    filename: &str,
    root_pos: TokenPosition,
    tokenized_result: &[SyntaxNode],
) -> Result<(), PositionedError> {
    let mut block = empty_block();
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
        build_symbol_child_type(),
        build_symbol(),
        &mut block,
    )?;
    let Some(main_fn) = main_fn else {
        return Err(throw_err(
            env,
            Some(root_pos),
            "expected main fn",
            None,
            None,
        ));
    };
    let void_raw = SyntaxNode::Raw(RawToken {
        pos: compiler_pos(),
        raw: String::new(),
        tag: RawTag::Void,
    });
    let call_result = analyze_call(
        env,
        std_folder_or_file_type(),
        root_pos.clone(),
        main_fn,
        CallArg {
            pos: compiler_pos(),
            ast: std::slice::from_ref(&void_raw),
        },
        &mut block,
    )?;
    let evaluated =
        crate::comptime::comptime_eval(env, &block, call_result.value, root_pos.clone())?;
    crate::comptime::get_comptime(
        env,
        Some(crate::comptime::ComptimeValueKind::BuildArtifact),
        RuntimeValue::Comptime(evaluated),
        root_pos,
    )?;
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

    let mut changes = HashMap::new();
    changes.insert(target_env_symbol(), TargetEnv::Build);
    let mut env = Env {
        trace: Vec::new(),
        errors: tokenized.errors.clone(),
        scope: Scope {
            comptime: ComptimeScopeMap::root(changes),
            bindings: HashMap::new(),
        },
        fn_cache: Rc::new(PerComptimeScopeCache::new()),
        decl_cache: Rc::new(PerComptimeScopeCache::new()),
        builtin_cache: Rc::new(PerComptimeScopeCache::new()),
    };

    if let Err(e) = import_file_body(&mut env, filename, root_pos, &tokenized.result) {
        handle_err(&mut env, e);
    }

    if env.errors.is_empty() {
        Ok(())
    } else {
        Err(env.errors)
    }
}
