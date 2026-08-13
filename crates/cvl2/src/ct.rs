#![allow(clippy::wrong_self_convention)]

use std::collections::HashMap;

use crate::backend::c::{validate_c_name, CValidatedIdentifierName};
use crate::compiler::{
    add_err, analyze, analyze_base, analyze_block, block_append, cast_value, create_declaration,
    empty_block, get_declaration, throw_consumed_err, throw_err, AnalysisBlock, AnalysisLine,
    AnalysisResult, Binary2, ComptimeFolder, ComptimeValue, ComptimeValueBuildArtifact,
    ComptimeValueCExportName, ComptimeValueDeclaration, ComptimeValueExportList,
    ComptimeValueExportListEntry, ComptimeValueKey, ComptimeValueMcIdentifier,
    ComptimeValueMcNbtRef, ComptimeValueMcResult, ComptimeValueUint8Array, ConsumedErrorToken, Env,
    PositionedError, RuntimeValue, Uint8ArraySourcemapEntry,
};
use crate::comptime::{comptime_eval, get_comptime, ComptimeValueKind};
use crate::parser::{
    unescape_string, BlockToken, ErrorStyle, IdentifierToken, RawTag, SyntaxNode, TokenPosition,
};
use crate::printers::printers::AST_NODE;

pub struct CallArg<'a> {
    pub pos: TokenPosition,
    pub ast: &'a [SyntaxNode],
}

#[derive(Debug, Clone)]
pub struct ImplicitArgRet {
    pub arg: Type,
    pub ret: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Void(TypeVoid),
    Unknown(TypeUnknown),
    Fn(TypeFn),
    Uint8Array(TypeUint8Array),
    Tuple(TypeTuple),
    Optional(TypeOptional),
    CtExportList(CtExportList),
    CtKey(CtKey),
    CtAst(CtAst),
    CtNamespace(CtNamespace),
    CtType(CtType),
    CtBuildArtifact(CtBuildArtifact),
    McResult(McResult),
    McNbtRef(McNbtRef),
    McIdentifier(McIdentifier),
    CExportName(CExportName),
}

impl Type {
    pub fn cast_into(
        &self,
        env: &mut Env,
        block: &mut AnalysisBlock,
        other: AnalysisResult,
        pos: TokenPosition,
    ) -> Result<AnalysisResult, PositionedError> {
        match self {
            Type::McResult(m) => m.cast_into(env, block, other, pos),
            _ => {
                if &other.ty == self {
                    Ok(other)
                } else {
                    Err(throw_err(
                        env,
                        Some(pos),
                        format!("TODO implicit casting: {}", self.dump()),
                        None,
                        None,
                    ))
                }
            }
        }
    }

    pub fn from_string(
        &self,
        env: &mut Env,
        slot: Type,
        ast: &BlockToken,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        match self {
            Type::Uint8Array(t) => t.from_string(env, slot, ast, block),
            Type::CtBuildArtifact(t) => t.from_string(env, slot, ast, block),
            Type::McNbtRef(t) => t.from_string(env, slot, ast, block),
            Type::McIdentifier(t) => t.from_string(env, slot, ast, block),
            Type::CExportName(t) => t.from_string(env, slot, ast, block),
            _ => Err(throw_err(
                env,
                Some(ast.pos.clone()),
                format!("String is not supported in slot: {}", self.dump()),
                None,
                None,
            )),
        }
    }

    pub fn from_map(
        &self,
        env: &mut Env,
        slot: Type,
        ast: &BlockToken,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        match self {
            Type::CtExportList(t) => t.from_map(env, slot, ast, block),
            Type::CtBuildArtifact(t) => t.from_map(env, slot, ast, block),
            _ => Err(throw_err(
                env,
                Some(ast.pos.clone()),
                format!("Map is not supported in slot: {}", self.dump()),
                None,
                None,
            )),
        }
    }

    pub fn from_number(
        &self,
        env: &mut Env,
        slot: Type,
        ast: &IdentifierToken,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        match self {
            Type::McResult(t) => t.from_number(env, slot, ast, block),
            _ => Err(throw_err(
                env,
                Some(ast.pos.clone()),
                format!("Number is not supported in slot: {}", self.dump()),
                None,
                None,
            )),
        }
    }

    pub fn dump(&self) -> String {
        match self {
            Type::Void(_) => "TypeVoid",
            Type::Unknown(_) => "TypeUnknown",
            Type::Fn(_) => "TypeFn",
            Type::Uint8Array(_) => "TypeUint8Array",
            Type::Tuple(_) => "TypeTuple",
            Type::Optional(_) => "TypeOptional",
            Type::CtExportList(_) => "CtExportList",
            Type::CtKey(_) => "CtKey",
            Type::CtAst(_) => "CtAst",
            Type::CtNamespace(_) => "CtNamespace",
            Type::CtType(_) => "CtType",
            Type::CtBuildArtifact(_) => "CtBuildArtifact",
            Type::McResult(_) => "McResult",
            Type::McNbtRef(_) => "McNbtRef",
            Type::McIdentifier(_) => "McIdentifier",
            Type::CExportName(_) => "CExportName",
        }
        .to_string()
    }

    pub fn analyze_call(
        &self,
        env: &mut Env,
        slot: Type,
        pos: TokenPosition,
        method: AnalysisResult,
        arg_in: CallArg,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        match self {
            Type::Fn(t) => t.analyze_call(env, slot, pos, method, arg_in, block),
            Type::CtNamespace(t) => t.analyze_call(env, slot, pos, method, arg_in, block),
            Type::CtType(t) => t.analyze_call(env, slot, pos, method, arg_in, block),
            _ => Err(throw_err(
                env,
                Some(pos),
                format!("not supported call type: {}", self.dump()),
                None,
                None,
            )),
        }
    }

    pub fn analyze_access(
        &self,
        env: &mut Env,
        slot: Type,
        obj: AnalysisResult,
        pos: TokenPosition,
        prop: AnalysisResult,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        match self {
            Type::CtNamespace(t) => t.analyze_access(env, slot, obj, pos, prop, block),
            _ => Err(throw_err(
                env,
                Some(pos),
                format!("not supported access type: {}", self.dump()),
                None,
                None,
            )),
        }
    }

    pub fn implicit_arg_ret_for_arrow_fn(&self) -> ImplicitArgRet {
        ImplicitArgRet {
            arg: Type::Unknown(TypeUnknown),
            ret: Type::Unknown(TypeUnknown),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeVoid;

#[derive(Debug, Clone, PartialEq)]
pub struct TypeUnknown;

#[derive(Debug, Clone, PartialEq)]
pub struct TypeFn {
    pub pos: TokenPosition,
    pub arg: Box<Type>,
    pub ret: Box<Type>,
}

impl TypeFn {
    fn analyze_call(
        &self,
        env: &mut Env,
        _slot: Type,
        pos: TokenPosition,
        method: AnalysisResult,
        arg_in: CallArg,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let arg = analyze(env, (*self.arg).clone(), arg_in.pos, arg_in.ast, block)?;
        let idx = block_append(
            block,
            AnalysisLine::Call {
                pos,
                method: method.value,
                arg: arg.value,
            },
        );
        Ok(AnalysisResult {
            ty: (*self.ret).clone(),
            value: RuntimeValue::Runtime(idx),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeUint8Array;

impl TypeUint8Array {
    fn from_string(
        &self,
        env: &mut Env,
        _slot: Type,
        ast: &BlockToken,
        _block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        if ast.items.len() != 1 {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                format!(
                    "TODO str items len != 1 todo{}",
                    AST_NODE.dump_list(&ast.items, 3)
                ),
                None,
                Some(ErrorStyle::Todo),
            ));
        }
        let it0 = &ast.items[0];
        let is_raw_string = matches!(it0, SyntaxNode::Raw(raw) if raw.tag == RawTag::String);
        if !is_raw_string {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                format!("TODO str item 0 ! raw string{}", AST_NODE.dump(it0, 3)),
                None,
                None,
            ));
        }
        let SyntaxNode::Raw(raw) = it0 else {
            unreachable!("just matched SyntaxNode::Raw above")
        };
        let unescaped = unescape_string(env, &raw.raw, raw.pos.clone())?;
        let sourcemap: Vec<Uint8ArraySourcemapEntry> = Vec::new();
        Ok(AnalysisResult {
            ty: Type::Uint8Array(TypeUint8Array),
            value: RuntimeValue::Comptime(ComptimeValue::Uint8Array(ComptimeValueUint8Array {
                value: unescaped.into_bytes(),
                sourcemap,
            })),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeTuple {
    pub children: Vec<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeOptional {
    pub child: Box<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CtExportList {
    pub key: Box<Type>,
}

impl CtExportList {
    fn from_map(
        &self,
        env: &mut Env,
        slot: Type,
        ast: &BlockToken,
        _block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let thiskey = (*self.key).clone();
        let mut exports_block = empty_block();
        let arr_entry_idx = block_append(
            &mut exports_block,
            AnalysisLine::ComptimeKvListInit {
                pos: ast.pos.clone(),
            },
        );
        let arr_entry = RuntimeValue::Runtime(arr_entry_idx);
        analyze_block(
            env,
            slot,
            ast.pos.clone(),
            &ast.items,
            &mut exports_block,
            |env, b2: Binary2, block| {
                let (lhs, op, rhs) = b2;
                let key = analyze(env, thiskey.clone(), lhs.pos.clone(), &lhs.items, block)?;
                let value = analyze(env, Type::CtAst(CtAst), rhs.pos.clone(), &rhs.items, block)?;
                let ret = block_append(
                    block,
                    AnalysisLine::ComptimeKvListAppend {
                        pos: op.pos.clone(),
                        list: arr_entry.clone(),
                        key: key.value,
                        value: value.value,
                    },
                );
                Ok(AnalysisResult {
                    ty: Type::Void(TypeVoid),
                    value: RuntimeValue::Runtime(ret),
                })
            },
        )?;

        let evaluated = comptime_eval(env, &exports_block, arr_entry, ast.pos.clone())?;
        let arr_value = get_comptime(
            env,
            Some(ComptimeValueKind::KvFields),
            RuntimeValue::Comptime(evaluated),
            ast.pos.clone(),
        )?;
        let ComptimeValue::KvFields(mut arr_value) = arr_value else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        arr_value.locked = true;

        let mut exports = Vec::new();
        for entry in arr_value.entries {
            let value = get_comptime(
                env,
                Some(ComptimeValueKind::Ast),
                RuntimeValue::Comptime(entry.value),
                entry.pos.clone(),
            )?;
            let ComptimeValue::Ast(value) = value else {
                unreachable!("get_comptime guarantees a matching kind")
            };
            exports.push(ComptimeValueExportListEntry {
                key: entry.key,
                key_pos: entry.pos,
                value,
            });
        }

        Ok(AnalysisResult {
            ty: Type::CtExportList(CtExportList {
                key: self.key.clone(),
            }),
            value: RuntimeValue::Comptime(ComptimeValue::ExportList(ComptimeValueExportList {
                exports,
            })),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CtKey;

#[derive(Debug, Clone, PartialEq)]
pub struct CtAst;

#[derive(Debug, Clone, PartialEq)]
pub struct CtNamespace;

impl CtNamespace {
    fn analyze_call(
        &self,
        env: &mut Env,
        slot: Type,
        pos: TokenPosition,
        method: AnalysisResult,
        arg_in: CallArg,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let value = get_comptime(
            env,
            Some(ComptimeValueKind::Namespace),
            method.value,
            pos.clone(),
        )?;
        let ComptimeValue::Namespace(ns) = value else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        let Some(call) = ns.call() else {
            return Err(throw_err(
                env,
                Some(pos.clone()),
                "this namespace does not support call",
                Some(vec![(Some(ns.pos().clone()), "defined here".to_string())]),
                None,
            ));
        };
        call(env, slot, pos, arg_in, block)
    }

    fn analyze_access(
        &self,
        env: &mut Env,
        _slot: Type,
        obj: AnalysisResult,
        pos: TokenPosition,
        prop: AnalysisResult,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let RuntimeValue::Comptime(ComptimeValue::Namespace(ns)) = &obj.value else {
            return Err(throw_err(
                env,
                Some(pos),
                format!(
                    "cannot access on namespace type with value kind {:?}",
                    obj.value
                ),
                None,
                None,
            ));
        };
        let as_key = Type::CtKey(CtKey).cast_into(env, block, prop, pos.clone())?;
        let kval = get_comptime(env, Some(ComptimeValueKind::Key), as_key.value, pos.clone())?;
        let ComptimeValue::Key(key) = kval else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        match key {
            ComptimeValueKey::String { key } => ns.get_string(env, pos, &key, block),
            ComptimeValueKey::Symbol { .. } => Err(throw_err(
                env,
                Some(pos),
                "TODO return ?symbolChildType .some(T) or .none",
                None,
                None,
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CtType;

impl CtType {
    fn analyze_call(
        &self,
        env: &mut Env,
        _slot: Type,
        pos: TokenPosition,
        method: AnalysisResult,
        arg_in: CallArg,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let slot_type = get_comptime(env, Some(ComptimeValueKind::Type), method.value, pos)?;
        let ComptimeValue::Type(slot_type) = slot_type else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        let result = analyze(env, slot_type.ty.clone(), arg_in.pos, arg_in.ast, block)?;
        Ok(cast_value(slot_type.ty, result))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CtBuildArtifactNarrow {
    Folder,
    File,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CtBuildArtifact {
    pub narrow: Option<CtBuildArtifactNarrow>,
}

enum FolderRegisteredEntry {
    Ok {
        decl: ComptimeValueDeclaration,
        pos: TokenPosition,
    },
    Error {
        etok: ConsumedErrorToken,
        pos: TokenPosition,
    },
}

impl CtBuildArtifact {
    fn from_string(
        &self,
        env: &mut Env,
        _slot: Type,
        ast: &BlockToken,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let str_result = analyze_base(
            env,
            Type::Uint8Array(TypeUint8Array),
            &SyntaxNode::Block(Box::new(ast.clone())),
            block,
        )?;
        let idx = block_append(
            block,
            AnalysisLine::ComptimeFileCreate {
                pos: ast.pos.clone(),
                value: str_result.value,
            },
        );
        Ok(AnalysisResult {
            ty: Type::CtBuildArtifact(CtBuildArtifact { narrow: None }),
            value: RuntimeValue::Runtime(idx),
        })
    }

    fn from_map(
        &self,
        env: &mut Env,
        slot: Type,
        ast: &BlockToken,
        _block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let mut exports_block = empty_block();
        let arr_entry_idx = block_append(
            &mut exports_block,
            AnalysisLine::ComptimeKvListInit {
                pos: ast.pos.clone(),
            },
        );
        let arr_entry = RuntimeValue::Runtime(arr_entry_idx);
        analyze_block(
            env,
            slot,
            ast.pos.clone(),
            &ast.items,
            &mut exports_block,
            |env, b2: Binary2, block| {
                let (lhs, op, rhs) = b2;
                let key = analyze(
                    env,
                    Type::Uint8Array(TypeUint8Array),
                    lhs.pos.clone(),
                    &lhs.items,
                    block,
                )?;
                let value = analyze(env, Type::CtAst(CtAst), rhs.pos.clone(), &rhs.items, block)?;
                let ret = block_append(
                    block,
                    AnalysisLine::ComptimeKvListAppend {
                        pos: op.pos.clone(),
                        list: arr_entry.clone(),
                        key: key.value,
                        value: value.value,
                    },
                );
                Ok(AnalysisResult {
                    ty: Type::Void(TypeVoid),
                    value: RuntimeValue::Runtime(ret),
                })
            },
        )?;

        let evaluated = comptime_eval(env, &exports_block, arr_entry, ast.pos.clone())?;
        let arr_value = get_comptime(
            env,
            Some(ComptimeValueKind::KvFields),
            RuntimeValue::Comptime(evaluated),
            ast.pos.clone(),
        )?;
        let ComptimeValue::KvFields(mut arr_value) = arr_value else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        arr_value.locked = true;

        let mut registered: HashMap<String, FolderRegisteredEntry> = HashMap::new();
        for entry in arr_value.entries {
            let raw_key = get_comptime(
                env,
                Some(ComptimeValueKind::Uint8Array),
                RuntimeValue::Comptime(entry.key),
                entry.pos.clone(),
            )?;
            let ComptimeValue::Uint8Array(raw_key) = raw_key else {
                unreachable!("get_comptime guarantees a matching kind")
            };
            let value = get_comptime(
                env,
                Some(ComptimeValueKind::Ast),
                RuntimeValue::Comptime(entry.value),
                entry.pos.clone(),
            )?;
            let ComptimeValue::Ast(value) = value else {
                unreachable!("get_comptime guarantees a matching kind")
            };
            let key = String::from_utf8_lossy(&raw_key.value).into_owned();
            if let Some(prev) = registered.get(&key) {
                let prev_pos = match prev {
                    FolderRegisteredEntry::Ok { pos, .. } => pos.clone(),
                    FolderRegisteredEntry::Error { pos, .. } => pos.clone(),
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
                    key,
                    FolderRegisteredEntry::Error {
                        etok,
                        pos: prev_pos,
                    },
                );
                continue;
            }
            registered.insert(
                key,
                FolderRegisteredEntry::Ok {
                    decl: create_declaration(env, value),
                    pos: entry.pos.clone(),
                },
            );
        }

        let mut result = HashMap::new();
        for (key, entry) in registered {
            match entry {
                FolderRegisteredEntry::Ok { decl, pos } => {
                    let declared = get_declaration(env, decl)?;
                    let subitm = get_comptime(
                        env,
                        Some(ComptimeValueKind::BuildArtifact),
                        RuntimeValue::Comptime(declared.value),
                        pos,
                    )?;
                    let ComptimeValue::BuildArtifact(subitm) = subitm else {
                        unreachable!("get_comptime guarantees a matching kind")
                    };
                    result.insert(key, subitm);
                }
                FolderRegisteredEntry::Error { etok, .. } => {
                    return Err(throw_consumed_err(etok));
                }
            }
        }

        Ok(AnalysisResult {
            ty: Type::CtBuildArtifact(CtBuildArtifact {
                narrow: Some(CtBuildArtifactNarrow::Folder),
            }),
            value: RuntimeValue::Comptime(ComptimeValue::BuildArtifact(
                ComptimeValueBuildArtifact::Folder(ComptimeFolder { value: result }),
            )),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McResultNarrow {
    I32,
    Fail,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McResult {
    pub narrow: Option<McResultNarrow>,
}

impl McResult {
    fn cast_into(
        &self,
        env: &mut Env,
        _block: &mut AnalysisBlock,
        other: AnalysisResult,
        pos: TokenPosition,
    ) -> Result<AnalysisResult, PositionedError> {
        let Type::McResult(other_narrow) = &other.ty else {
            return Err(throw_err(
                env,
                Some(pos),
                "no implicit cast available",
                None,
                None,
            ));
        };
        if self.narrow.is_some() && self.narrow != other_narrow.narrow {
            return Err(throw_err(env, Some(pos), "cannot widen", None, None));
        }
        Ok(other)
    }

    fn from_number(
        &self,
        env: &mut Env,
        _slot: Type,
        ast: &IdentifierToken,
        _block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let parsed = ast
            .str
            .parse::<i32>()
            .ok()
            .filter(|v| v.to_string() == ast.str);
        let Some(parsed) = parsed else {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                format!("invalid i32: got '{}'", ast.str),
                None,
                None,
            ));
        };
        Ok(AnalysisResult {
            ty: Type::McResult(McResult {
                narrow: Some(McResultNarrow::I32),
            }),
            value: RuntimeValue::Comptime(ComptimeValue::McResult(ComptimeValueMcResult {
                result: parsed,
            })),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum McNbtRefNarrow {
    String,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    List(Box<McNbtRef>),
    Compound(HashMap<String, McNbtRef>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct McNbtRef {
    pub narrow: Option<McNbtRefNarrow>,
}

impl McNbtRef {
    fn from_string(
        &self,
        env: &mut Env,
        _slot: Type,
        ast: &BlockToken,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let str_result = analyze_base(
            env,
            Type::Uint8Array(TypeUint8Array),
            &SyntaxNode::Block(Box::new(ast.clone())),
            block,
        )?;
        let u8a = get_comptime(
            env,
            Some(ComptimeValueKind::Uint8Array),
            str_result.value,
            ast.pos.clone(),
        )?;
        let ComptimeValue::Uint8Array(u8a) = u8a else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        let decoded = String::from_utf8_lossy(&u8a.value).into_owned();
        Ok(AnalysisResult {
            ty: Type::McNbtRef(McNbtRef {
                narrow: Some(McNbtRefNarrow::String),
            }),
            value: RuntimeValue::Comptime(ComptimeValue::McNbtRef(ComptimeValueMcNbtRef::String(
                decoded,
            ))),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct McIdentifier {
    pub category: Option<String>,
}

impl McIdentifier {
    fn from_string(
        &self,
        env: &mut Env,
        _slot: Type,
        ast: &BlockToken,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let str_result = analyze_base(
            env,
            Type::Uint8Array(TypeUint8Array),
            &SyntaxNode::Block(Box::new(ast.clone())),
            block,
        )?;
        let u8a = get_comptime(
            env,
            Some(ComptimeValueKind::Uint8Array),
            str_result.value,
            ast.pos.clone(),
        )?;
        let ComptimeValue::Uint8Array(u8a) = u8a else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        let decoded = String::from_utf8_lossy(&u8a.value).into_owned();
        let Some((namespace, path)) = parse_mc_identifier(&decoded) else {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                "invalid minecraft identifier name",
                None,
                None,
            ));
        };
        if namespace == ".." {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                "invalid minecraft identifier name",
                None,
                None,
            ));
        }
        Ok(AnalysisResult {
            ty: Type::McIdentifier(McIdentifier { category: None }),
            value: RuntimeValue::Comptime(ComptimeValue::McIdentifier(ComptimeValueMcIdentifier {
                namespace,
                path,
            })),
        })
    }
}

fn parse_mc_identifier(decoded: &str) -> Option<(String, String)> {
    fn is_id_char(c: char) -> bool {
        matches!(c, '-' | '.' | '_' | '0'..='9' | 'a'..='z')
    }
    let (namespace_part, path_part) = match decoded.split_once(':') {
        Some((ns, path)) => (Some(ns), path),
        None => (None, decoded),
    };
    if let Some(ns) = namespace_part {
        if ns.is_empty() || !ns.chars().all(is_id_char) {
            return None;
        }
    }
    if path_part.is_empty() || !path_part.chars().all(|c| is_id_char(c) || c == '/') {
        return None;
    }
    Some((
        namespace_part.unwrap_or("minecraft").to_string(),
        path_part.to_string(),
    ))
}

#[derive(Debug, Clone, PartialEq)]
pub struct CExportName;

impl CExportName {
    fn from_string(
        &self,
        env: &mut Env,
        _slot: Type,
        ast: &BlockToken,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let str_result = analyze_base(
            env,
            Type::Uint8Array(TypeUint8Array),
            &SyntaxNode::Block(Box::new(ast.clone())),
            block,
        )?;
        let u8a = get_comptime(
            env,
            Some(ComptimeValueKind::Uint8Array),
            str_result.value,
            ast.pos.clone(),
        )?;
        let ComptimeValue::Uint8Array(u8a) = u8a else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        let decoded = String::from_utf8_lossy(&u8a.value).into_owned();
        if !validate_c_name(&decoded) {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                "invalid c identifier name",
                None,
                None,
            ));
        }
        let Some(validated) = CValidatedIdentifierName::new(decoded) else {
            unreachable!("validate_c_name already confirmed this name is valid")
        };
        Ok(AnalysisResult {
            ty: Type::CExportName(CExportName),
            value: RuntimeValue::Comptime(ComptimeValue::CExportName(ComptimeValueCExportName {
                value: validated,
            })),
        })
    }
}
