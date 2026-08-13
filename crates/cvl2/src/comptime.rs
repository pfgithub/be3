use crate::compiler::{
    analyze_function, throw_consumed_err, throw_err, AnalysisBlock, AnalysisLine, ComptimeFile,
    ComptimeValue, ComptimeValueBuildArtifact, Env, NsFields, NsFieldsEntry, PositionedError,
    RuntimeValue,
};
use crate::parser::{ErrorStyle, TokenPosition};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComptimeValueKind {
    Key,
    Namespace,
    Type,
    Ast,
    Void,
    KvFields,
    Fn,
    Optional,
    BuildArtifact,
    Uint8Array,
    ExportList,
    CExportName,
    McIdentifier,
    McResult,
    McNbtRef,
    Error,
    Mc,
}

impl ComptimeValueKind {
    fn of(value: &ComptimeValue) -> Self {
        match value {
            ComptimeValue::Key(_) => ComptimeValueKind::Key,
            ComptimeValue::Namespace(_) => ComptimeValueKind::Namespace,
            ComptimeValue::Type(_) => ComptimeValueKind::Type,
            ComptimeValue::Ast(_) => ComptimeValueKind::Ast,
            ComptimeValue::Void(_) => ComptimeValueKind::Void,
            ComptimeValue::KvFields(_) => ComptimeValueKind::KvFields,
            ComptimeValue::Fn(_) => ComptimeValueKind::Fn,
            ComptimeValue::Optional(_) => ComptimeValueKind::Optional,
            ComptimeValue::BuildArtifact(_) => ComptimeValueKind::BuildArtifact,
            ComptimeValue::Uint8Array(_) => ComptimeValueKind::Uint8Array,
            ComptimeValue::ExportList(_) => ComptimeValueKind::ExportList,
            ComptimeValue::CExportName(_) => ComptimeValueKind::CExportName,
            ComptimeValue::McIdentifier(_) => ComptimeValueKind::McIdentifier,
            ComptimeValue::McResult(_) => ComptimeValueKind::McResult,
            ComptimeValue::McNbtRef(_) => ComptimeValueKind::McNbtRef,
            ComptimeValue::Error(_) => ComptimeValueKind::Error,
            ComptimeValue::Mc(_) => ComptimeValueKind::Mc,
        }
    }
}

struct RuntimeData<'a> {
    block: &'a AnalysisBlock,
    results: &'a [Option<ComptimeValue>],
}

fn narrow_mismatch(
    env: &Env,
    pos: TokenPosition,
    expected: ComptimeValueKind,
    actual: ComptimeValueKind,
) -> PositionedError {
    throw_err(
        env,
        Some(pos),
        format!("Expected value of type {expected:?}, got {actual:?}"),
        None,
        None,
    )
}

fn get_comptime_impl(
    env: &mut Env,
    k: Option<ComptimeValueKind>,
    v: RuntimeValue,
    pos: TokenPosition,
    runtime: Option<&RuntimeData<'_>>,
) -> Result<ComptimeValue, PositionedError> {
    match v {
        RuntimeValue::Runtime(idx) => {
            let Some(runtime) = runtime else {
                return Err(throw_err(
                    env,
                    Some(pos),
                    "Value must be known at comptime.",
                    None,
                    None,
                ));
            };
            if idx.1 != runtime.block.validate {
                return Err(throw_err(
                    env,
                    Some(pos),
                    "Assertion failure: Ex",
                    None,
                    None,
                ));
            }
            let resolved = runtime.results[idx.0]
                .clone()
                .expect("runtime result must be populated before being referenced");
            get_comptime_impl(env, k, RuntimeValue::Comptime(resolved), pos, None)
        }
        RuntimeValue::Comptime(ComptimeValue::Error(err))
            if k.is_some_and(|k| k != ComptimeValueKind::Error) =>
        {
            Err(throw_consumed_err(err.etok))
        }
        RuntimeValue::Comptime(value) => match k {
            None => Ok(value),
            Some(k) if ComptimeValueKind::of(&value) == k => Ok(value),
            Some(k) => Err(narrow_mismatch(env, pos, k, ComptimeValueKind::of(&value))),
        },
    }
}

pub fn get_comptime(
    env: &mut Env,
    k: Option<ComptimeValueKind>,
    v: RuntimeValue,
    pos: TokenPosition,
) -> Result<ComptimeValue, PositionedError> {
    get_comptime_impl(env, k, v, pos, None)
}

pub fn comptime_eval(
    env: &mut Env,
    block: &AnalysisBlock,
    result: RuntimeValue,
    pos: TokenPosition,
) -> Result<ComptimeValue, PositionedError> {
    comptime_eval_with_args(env, block, result, pos, None)
}

fn comptime_eval_with_args(
    env: &mut Env,
    block: &AnalysisBlock,
    result: RuntimeValue,
    pos: TokenPosition,
    args: Option<RuntimeValue>,
) -> Result<ComptimeValue, PositionedError> {
    let mut results: Vec<Option<ComptimeValue>> = block.lines.iter().map(|_| None).collect();

    for (i, instr) in block.lines.iter().enumerate() {
        match instr {
            AnalysisLine::ComptimeKvListInit { .. } => {
                results[i] = Some(ComptimeValue::KvFields(NsFields {
                    locked: false,
                    entries: Vec::new(),
                }));
            }
            AnalysisLine::ComptimeKvListAppend {
                pos: ipos,
                key,
                list,
                value,
            } => {
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                let fields_check = get_comptime_impl(
                    env,
                    Some(ComptimeValueKind::KvFields),
                    list.clone(),
                    ipos.clone(),
                    Some(&runtime),
                )?;
                let ComptimeValue::KvFields(fields_check) = fields_check else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                if fields_check.locked {
                    return Err(throw_err(
                        env,
                        Some(ipos.clone()),
                        "assertion failed",
                        None,
                        None,
                    ));
                }
                let key_val =
                    get_comptime_impl(env, None, key.clone(), ipos.clone(), Some(&runtime))?;
                let value_val =
                    get_comptime_impl(env, None, value.clone(), ipos.clone(), Some(&runtime))?;

                let RuntimeValue::Runtime(list_idx) = list else {
                    unreachable!("get_comptime already validated this resolves to a runtime slot")
                };
                let Some(ComptimeValue::KvFields(fields)) = &mut results[list_idx.0] else {
                    unreachable!("validated above via get_comptime")
                };
                fields.entries.push(NsFieldsEntry {
                    pos: ipos.clone(),
                    key: key_val,
                    value: value_val,
                });
            }
            AnalysisLine::Call {
                pos: ipos,
                method,
                arg,
            } => {
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                let method_val = get_comptime_impl(
                    env,
                    Some(ComptimeValueKind::Fn),
                    method.clone(),
                    ipos.clone(),
                    Some(&runtime),
                )?;
                let ComptimeValue::Fn(method_val) = method_val else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                let arg_val =
                    get_comptime_impl(env, None, arg.clone(), ipos.clone(), Some(&runtime))?;
                let body = analyze_function(env, &method_val)?;
                let value = comptime_eval_with_args(
                    env,
                    &body.block,
                    body.value,
                    ipos.clone(),
                    Some(RuntimeValue::Comptime(arg_val)),
                )?;
                results[i] = Some(value);
            }
            AnalysisLine::Args { pos: ipos } => {
                let Some(args_val) = args.clone() else {
                    return Err(throw_err(
                        env,
                        Some(ipos.clone()),
                        "cannot get args when executing without args",
                        Some(vec![(Some(pos.clone()), "called here".to_string())]),
                        Some(ErrorStyle::Unreachable),
                    ));
                };
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                let value = get_comptime_impl(env, None, args_val, ipos.clone(), Some(&runtime))?;
                results[i] = Some(value);
            }
            AnalysisLine::ComptimeFileCreate { pos: ipos, value } => {
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                let arg_val = get_comptime_impl(
                    env,
                    Some(ComptimeValueKind::Uint8Array),
                    value.clone(),
                    ipos.clone(),
                    Some(&runtime),
                )?;
                let ComptimeValue::Uint8Array(arg_val) = arg_val else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                results[i] = Some(ComptimeValue::BuildArtifact(
                    ComptimeValueBuildArtifact::File(ComptimeFile {
                        value: arg_val.value,
                    }),
                ));
            }
            AnalysisLine::Break { pos: ipos, .. } => {
                return Err(throw_err(
                    env,
                    Some(ipos.clone()),
                    "todo: comptime eval expr: break",
                    None,
                    None,
                ));
            }
            AnalysisLine::McExecRaw { pos: ipos, .. } => {
                return Err(throw_err(
                    env,
                    Some(ipos.clone()),
                    "todo: comptime eval expr: mc:exec_raw",
                    None,
                    None,
                ));
            }
        }
    }

    let runtime = RuntimeData {
        block,
        results: &results,
    };
    get_comptime_impl(env, None, result, pos, Some(&runtime))
}
