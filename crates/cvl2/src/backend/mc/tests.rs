use super::*;
use crate::compiler::{
    AnalysisBlock, AnalysisLine, BlockIdx, ComptimeScopeMap, ComptimeValue, ComptimeValueAst,
    ComptimeValueMcNbtRef, ComptimeValueMcResult, ComptimeValueVoid, Destructure,
    DestructureExtract, Env, PerComptimeScopeCache, PositionedError, RuntimeValue, Scope, Symbol,
};
use crate::ct::{Type, TypeUnknown};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

fn new_env() -> Env {
    Env {
        trace: Vec::new(),
        errors: Vec::new(),
        scope: Scope {
            comptime: ComptimeScopeMap::root(HashMap::new()),
            bindings: Rc::new(RefCell::new(HashMap::new())),
        },
        fn_cache: Rc::new(PerComptimeScopeCache::new()),
        decl_cache: Rc::new(PerComptimeScopeCache::new()),
        builtin_cache: Rc::new(PerComptimeScopeCache::new()),
    }
}

fn pos_at(idx: usize) -> TokenPosition {
    TokenPosition {
        fyl: "test".to_string(),
        idx,
        lyn: 1,
        col: idx + 1,
    }
}

fn dummy_fn(env: &Env, pos: TokenPosition) -> ComptimeValueFn {
    let destructure = Destructure {
        targets: Vec::new(),
        extract: DestructureExtract::Discard { pos: pos.clone() },
        ty: Type::Unknown(TypeUnknown),
        tags: Vec::new(),
    };
    let body = ComptimeValueAst {
        ast: Vec::new(),
        pos: pos.clone(),
        scope: env.scope.clone(),
    };
    ComptimeValueFn::new(destructure, body, pos)
}

mod codegen_mcfunction_appends_return_result_after_raw_command;
mod codegen_mcfunction_dedups_repeated_function_calls;
mod codegen_mcfunction_emits_raw_command_as_return_run;
mod codegen_mcfunction_errors_on_unsupported_line_kind;
mod codegen_mcfunction_errors_when_result_was_lost;
