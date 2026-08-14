use super::*;
use crate::compiler::{
    BlockIdx, ComptimeScopeMap, ComptimeValueError, ComptimeValueKey, ComptimeValueUint8Array,
    ComptimeValueVoid, ConsumedErrorToken, PerComptimeScopeCache, Scope, Symbol,
};
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

mod comptime_eval_args_line_resolves_supplied_argument;
mod comptime_eval_args_line_without_args_errors;
mod comptime_eval_file_create_wraps_bytes_in_build_artifact;
mod comptime_eval_kv_list_append_rejects_locked_fields;
mod comptime_eval_kv_list_init_and_append_builds_fields;
mod comptime_eval_runtime_value_from_other_block_errors;
mod comptime_eval_unhandled_break_line_errors;
mod get_comptime_consumes_error_value_when_kind_requested;
mod get_comptime_reports_kind_mismatch;
mod get_comptime_resolves_matching_comptime_value;
mod get_comptime_runtime_value_outside_comptime_eval_errors;
