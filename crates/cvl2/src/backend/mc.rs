use std::collections::HashMap;

use crate::compiler::{
    compiler_pos, throw_err, AnalysisBlock, AnalysisLine, ComptimeValue, ComptimeValueFn,
    ComptimeValueMcIdentifier, ComptimeValueMcNbtRef, Env, PositionedError, RuntimeValue,
};
use crate::comptime::{get_comptime, ComptimeValueKind};
use crate::parser::TokenPosition;
use crate::printers::printers::{BLOCK, RUNTIME_VALUE};
use crate::printers::{analysis_line_pos, UNLIMITED_DEPTH};

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub struct McCodegenCtx {
    pub fns: HashMap<ComptimeValueFn, ComptimeValueMcIdentifier>,
    pub gid: u64,
    pub internal_ns: String,
}

fn get_fn_name(ctx: &mut McCodegenCtx, fn_val: &ComptimeValueFn) -> ComptimeValueMcIdentifier {
    if let Some(existing) = ctx.fns.get(fn_val) {
        return existing.clone();
    }
    let res = ComptimeValueMcIdentifier {
        namespace: ctx.internal_ns.clone(),
        path: format!("_{}", ctx.gid),
    };
    ctx.gid += 1;
    ctx.fns.insert(fn_val.clone(), res.clone());
    res
}

struct UncommittedLine {
    idx: Option<usize>,
    cmd: String,
}

struct LineBuilder {
    raw_lines: Vec<String>,
    lost_positions: Vec<Option<TokenPosition>>,
    uncommitted: Option<UncommittedLine>,
}

impl LineBuilder {
    fn new(len: usize) -> Self {
        LineBuilder {
            raw_lines: Vec::new(),
            lost_positions: vec![None; len],
            uncommitted: None,
        }
    }

    fn add_line(&mut self, idx: Option<usize>, pos: TokenPosition, cmd: String) {
        if let Some(prev) = self.uncommitted.take() {
            self.raw_lines.push(prev.cmd);
            if let Some(prev_idx) = prev.idx {
                self.lost_positions[prev_idx] = Some(pos);
            }
        }
        self.uncommitted = Some(UncommittedLine { idx, cmd });
    }

    fn finish(mut self) -> String {
        if let Some(u) = self.uncommitted.take() {
            self.raw_lines.push(u.cmd);
        }
        self.raw_lines.join("\n")
    }
}

pub fn codegen_mcfunction(
    env: &mut Env,
    ctx: &mut McCodegenCtx,
    block: &AnalysisBlock,
    value: RuntimeValue,
) -> Result<String, PositionedError> {
    let mut builder = LineBuilder::new(block.lines.len());

    for (i, line) in block.lines.iter().enumerate() {
        match line {
            AnalysisLine::Args { .. } => {}
            AnalysisLine::Call { pos, method, .. } => {
                let method_ct = get_comptime(
                    env,
                    Some(ComptimeValueKind::Fn),
                    method.clone(),
                    pos.clone(),
                )?;
                let ComptimeValue::Fn(method_fn) = method_ct else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                let method_name = get_fn_name(ctx, &method_fn);
                builder.add_line(
                    Some(i),
                    pos.clone(),
                    format!("function {}:{}", method_name.namespace, method_name.path),
                );
            }
            AnalysisLine::McExecRaw { pos, command } => {
                let exec_value = get_comptime(
                    env,
                    Some(ComptimeValueKind::McNbtRef),
                    command.clone(),
                    pos.clone(),
                )?;
                let ComptimeValue::McNbtRef(ComptimeValueMcNbtRef::String(cmd)) = exec_value else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                builder.add_line(Some(i), pos.clone(), cmd);
            }
            AnalysisLine::ComptimeKvListInit { pos }
            | AnalysisLine::ComptimeKvListAppend { pos, .. }
            | AnalysisLine::Break { pos, .. }
            | AnalysisLine::ComptimeFileCreate { pos, .. } => {
                return Err(throw_err(
                    env,
                    Some(pos.clone()),
                    format!(
                        "TODO codegenMcfunction line: {}",
                        BLOCK.dump(block, UNLIMITED_DEPTH)
                    ),
                    None,
                    None,
                ));
            }
        }
    }

    match value {
        RuntimeValue::Comptime(ComptimeValue::McResult(result)) => {
            builder.add_line(None, compiler_pos(), format!("return {}", result.result));
        }
        RuntimeValue::Runtime(idx) => {
            if idx.1 != block.validate {
                return Err(throw_err(
                    env,
                    Some(compiler_pos()),
                    "assertion failed: Ex",
                    None,
                    None,
                ));
            }
            let matches_uncommitted =
                builder.uncommitted.as_ref().and_then(|u| u.idx) == Some(idx.0);
            if matches_uncommitted {
                let uncommitted = builder
                    .uncommitted
                    .as_mut()
                    .expect("matches_uncommitted implies uncommitted is Some");
                uncommitted.cmd = format!("return run {}", uncommitted.cmd);
            } else {
                return Err(throw_err(
                    env,
                    Some(compiler_pos()),
                    "this result was lost",
                    Some(vec![
                        (
                            Some(analysis_line_pos(&block.lines[idx.0]).clone()),
                            "acquired here".to_string(),
                        ),
                        (
                            Some(
                                builder.lost_positions[idx.0]
                                    .clone()
                                    .unwrap_or_else(compiler_pos),
                            ),
                            "lost here".to_string(),
                        ),
                    ]),
                    None,
                ));
            }
        }
        other => {
            return Err(throw_err(
                env,
                Some(compiler_pos()),
                format!(
                    "TODO codegenMcfunction result: {}",
                    RUNTIME_VALUE.dump(&other, UNLIMITED_DEPTH)
                ),
                None,
                None,
            ));
        }
    }

    Ok(builder.finish())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McEntitiesRefMain {
    S,
    P,
    A,
    R,
    E,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueMcEntitiesRef {
    pub main: McEntitiesRefMain,
    pub parameters: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McPositionAnchor {
    Eyes,
    Feet,
}

#[derive(Debug, Clone)]
pub enum ComptimeValueMcPositionRef {
    AbsRel {
        x: f64,
        x_rel: bool,
        y: f64,
        y_rel: bool,
        z: f64,
        z_rel: bool,
    },
    Caret {
        x: f64,
        y: f64,
        z: f64,
        anchor: McPositionAnchor,
    },
}

#[derive(Debug, Clone)]
pub enum ComptimeValueMcRotationRef {
    AbsRel {
        x: f64,
        x_rel: f64,
        y: f64,
        y_rel: f64,
    },
    As {
        selector: ComptimeValueMcEntitiesRef,
    },
}

#[derive(Debug, Clone)]
pub struct ComptimeValueMcLocation {
    pub position: ComptimeValueMcPositionRef,
    pub rotation: ComptimeValueMcRotationRef,
    pub dimension: Option<ComptimeValueMcIdentifier>,
}

#[derive(Debug, Clone)]
pub enum ComptimeValueMc {
    EntitiesRef(ComptimeValueMcEntitiesRef),
    PositionRef(ComptimeValueMcPositionRef),
    Location(ComptimeValueMcLocation),
    RotationRef(ComptimeValueMcRotationRef),
}
