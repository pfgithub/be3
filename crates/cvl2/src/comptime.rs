use std::any::Any;
use std::collections::HashMap;

use crate::compiler::{
    assert, syntax_node_kind, throw_err, AnalysisBlock, AnalysisLine, ComptimeNarrowKey,
    ComptimeType, ComptimeValueAst, Destructure, DestructureExtract, Env, NsFields, NsKey,
    PositionedError, RegisteredEntry,
};
use crate::parser::{colors, BracketTag, IdentifierTag, OpTag, RawTag, SyntaxNode, TokenPosition};

#[cfg(test)]
mod tests;

fn analysis_line_tag(line: &AnalysisLine) -> &'static str {
    match line {
        AnalysisLine::ComptimeOnly { .. } => "comptime:only",
        AnalysisLine::ComptimeNsListInit { .. } => "comptime:ns_list_init",
        AnalysisLine::ComptimeKey { .. } => "comptime:key",
        AnalysisLine::ComptimeAst { .. } => "comptime:ast",
        AnalysisLine::ComptimeNsListAppend { .. } => "comptime:ns_list_append",
        AnalysisLine::Void { .. } => "void",
        AnalysisLine::Call { .. } => "call",
        AnalysisLine::Break { .. } => "break",
    }
}

pub fn comptime_eval(
    env: &mut Env,
    block: &AnalysisBlock,
) -> Result<Vec<Box<dyn Any>>, PositionedError> {
    let mut results: Vec<Box<dyn Any>> = Vec::with_capacity(block.lines.len());
    for instr in &block.lines {
        let result: Box<dyn Any> = match instr {
            AnalysisLine::Void { .. } => Box::new(()),
            AnalysisLine::ComptimeOnly { .. } => Box::new(()),
            AnalysisLine::ComptimeAst { narrow, .. } => Box::new(narrow.clone()),
            AnalysisLine::ComptimeKey { narrow, .. } => Box::new(narrow.clone()),
            AnalysisLine::ComptimeNsListInit { .. } => Box::new(NsFields {
                locked: false,
                registered: HashMap::new(),
            }),
            AnalysisLine::ComptimeNsListAppend {
                key, list, value, ..
            } => {
                let ast = results[value.0]
                    .downcast_ref::<ComptimeValueAst>()
                    .expect("comptime:ns_list_append value must be ComptimeValueAst")
                    .clone();
                let key_narrow = results[key.0]
                    .downcast_ref::<ComptimeNarrowKey>()
                    .expect("comptime:ns_list_append key must be ComptimeNarrowKey")
                    .clone();
                let ns_key = match &key_narrow {
                    ComptimeNarrowKey::String { key } => NsKey::Str(key.clone()),
                    ComptimeNarrowKey::Symbol { key, .. } => NsKey::Sym(*key),
                };

                let fields = results[list.0]
                    .downcast_mut::<NsFields>()
                    .expect("comptime:ns_list_append list must be NsFields");
                assert(!fields.locked);

                if let Some(prevdef) = fields.registered.get(&ns_key) {
                    return Err(throw_err(
                        env,
                        Some(ast.pos.clone()),
                        "already declared",
                        Some(vec![(
                            Some(prevdef.ast.pos.clone()),
                            "previous definition here".to_string(),
                        )]),
                    ));
                }
                fields.registered.insert(
                    ns_key,
                    RegisteredEntry {
                        key: key_narrow,
                        ast,
                    },
                );

                Box::new(())
            }
            AnalysisLine::Call { .. } | AnalysisLine::Break { .. } => {
                panic!("todo: comptime eval expr: {}", analysis_line_tag(instr))
            }
        };
        results.push(result);
    }
    Ok(results)
}

struct PrintCfg {
    indent: String,
}

struct Adisp {
    cfg: PrintCfg,
    indent_count: usize,
    depth: usize,
    res: Vec<String>,
}

impl Adisp {
    fn new(depth: usize) -> Self {
        Adisp {
            cfg: PrintCfg {
                indent: "\u{2502} ".to_string(),
            },
            indent_count: 0,
            depth,
            res: Vec::new(),
        }
    }

    fn end(self) -> String {
        self.res.concat()
    }

    fn put(&mut self, msg: &str, color: Option<&str>) {
        if let Some(c) = color {
            self.res.push(c.to_string());
        }
        self.res.push(msg.to_string());
        if color.is_some() {
            self.res.push(colors::RESET.to_string());
        }
    }

    fn put_newline(&mut self) {
        self.put("\n", None);
        let indent = self.cfg.indent.repeat(self.indent_count);
        self.put(&indent, Some(colors::BLACK));
    }

    fn put_src(&mut self, pos: &TokenPosition) {
        let msg = format!(" \u{b7} {}:{}:{}", pos.fyl, pos.lyn, pos.col);
        self.put(&msg, Some(colors::BLACK));
    }

    fn put_check_depth(&mut self, n: Option<usize>) -> bool {
        if self.indent_count > self.depth {
            self.put_newline();
            self.put("...", Some(colors::BLACK));
            if let Some(n) = n {
                if n > 0 {
                    let msg = format!(" {n} item{}", if n == 1 { "" } else { "s" });
                    self.put(&msg, Some(colors::BLACK));
                }
            }
            return true;
        }
        false
    }

    fn put_single<T>(&mut self, printer: fn(&mut Adisp, &T), value: &T) {
        self.indent_count += 1;
        if self.put_check_depth(None) {
            self.indent_count -= 1;
            return;
        }
        self.put_newline();
        printer(self, value);
        self.indent_count -= 1;
    }

    fn put_multi<T>(&mut self, printer: fn(&mut Adisp, &T), value: &T) {
        self.indent_count += 1;
        if self.put_check_depth(None) {
            self.indent_count -= 1;
            return;
        }
        printer(self, value);
        self.indent_count -= 1;
    }

    fn put_list<T>(&mut self, printer: fn(&mut Adisp, &T), children: &[T]) {
        self.indent_count += 1;
        if children.is_empty() {
            self.put_newline();
            self.put("*no children*", Some(colors::BLACK));
            self.indent_count -= 1;
            return;
        }
        if self.put_check_depth(Some(children.len())) {
            self.indent_count -= 1;
            return;
        }
        for child in children {
            self.put_newline();
            printer(self, child);
        }
        self.indent_count -= 1;
    }
}

// Rust's `{:?}` string formatting doesn't match JS's `JSON.stringify` escaping
// rules, so this hand-rolls the subset that matters for this toy language's
// identifiers/strings: quotes, backslashes, the common short escapes, and a
// `\u00XX` fallback for other control characters. It doesn't reproduce
// JSON.stringify's UTF-16 surrogate-pair splitting for astral characters,
// which isn't meaningful for Rust's `char` and doesn't come up in practice
// here.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn is_simple_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
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

fn bracket_tag_str(tag: BracketTag) -> &'static str {
    match tag {
        BracketTag::Map => "map",
        BracketTag::List => "list",
        BracketTag::Code => "code",
        BracketTag::ColonCall => "colon_call",
        BracketTag::ArrowFn => "arrow_fn",
        BracketTag::String => "string",
        BracketTag::None => "",
    }
}

fn ident_tag_str(tag: IdentifierTag) -> &'static str {
    match tag {
        IdentifierTag::Normal => "normal",
        IdentifierTag::Access => "access",
        IdentifierTag::Builtin => "builtin",
    }
}

fn raw_tag_str(tag: RawTag) -> &'static str {
    match tag {
        RawTag::Return => "return",
        RawTag::Discard => "discard",
    }
}

fn put_block(adisp: &mut Adisp, block: &AnalysisBlock) {
    if adisp.put_check_depth(None) {
        return;
    }
    for (i, expr) in block.lines.iter().enumerate() {
        adisp.put_newline();
        adisp.put(&format!("{i} = "), None);
        adisp.put(analysis_line_tag(expr), Some(colors::MAGENTA));
        match expr {
            AnalysisLine::Call { pos, method, arg } => {
                adisp.put(&format!(" method={} arg={}", method.0, arg.0), None);
                adisp.put_src(pos);
            }
            AnalysisLine::ComptimeOnly { pos } => {
                adisp.put_src(pos);
            }
            AnalysisLine::ComptimeNsListInit { pos } => {
                adisp.put_src(pos);
            }
            AnalysisLine::ComptimeKey { pos, narrow } => match narrow {
                ComptimeNarrowKey::String { key } => {
                    adisp.put(&format!(" str={}", json_string(key)), None);
                    adisp.put_src(pos);
                }
                ComptimeNarrowKey::Symbol { child, .. } => {
                    // `Symbol` in this port carries no description string (see
                    // module docs on `Env`/`Symbol` in compiler.rs), so this
                    // always prints "(unnamed)" like the TS fallback does for
                    // every symbol actually created in this codebase today.
                    adisp.put(&format!(" str={}, type=", json_string("(unnamed)")), None);
                    adisp.put_src(pos);
                    adisp.put_single(put_type, child);
                }
            },
            AnalysisLine::ComptimeAst { pos, narrow } => {
                adisp.put(" ast:", None);
                adisp.put_src(pos);
                adisp.put_list(put_ast_node, &narrow.ast);
            }
            AnalysisLine::ComptimeNsListAppend {
                pos,
                key,
                list,
                value,
            } => {
                adisp.put(
                    &format!(" key={} list={} value={}", key.0, list.0, value.0),
                    None,
                );
                adisp.put_src(pos);
            }
            AnalysisLine::Void { pos } | AnalysisLine::Break { pos, .. } => {
                adisp.put(" %%TODO%%", None);
                adisp.put_src(pos);
            }
        }
    }
}

fn put_destructure(adisp: &mut Adisp, destructure: &Destructure) {
    adisp.put_newline();
    adisp.put("extract=", None);
    adisp.put_single(put_destructure_extract, &destructure.extract);
    adisp.put_newline();
    adisp.put("type=", None);
    adisp.put_single(put_type, &destructure.ty);
}

fn put_destructure_extract(adisp: &mut Adisp, extract: &DestructureExtract) {
    adisp.put(extract.tag(), Some(colors::CYAN));
    match extract {
        DestructureExtract::SingleItem { name, pos } => {
            adisp.put(&format!(" {}", json_string(name)), Some(colors::GREEN));
            adisp.put_src(pos);
        }
        DestructureExtract::List { items, pos } => {
            adisp.put_src(pos);
            adisp.put_list(put_destructure_extract, items);
        }
        DestructureExtract::Map { pos, .. } => {
            adisp.put(" %%TODO%%", None);
            adisp.put_src(pos);
        }
    }
}

fn put_type(adisp: &mut Adisp, ty: &ComptimeType) {
    adisp.put(ty.tag(), Some(colors::YELLOW));
    match ty {
        ComptimeType::Fn(f) => {
            adisp.put_src(&f.pos);
            adisp.indent_count += 1;
            adisp.put_newline();
            adisp.put("arg=", None);
            adisp.put_single(put_type, f.arg.as_ref());
            adisp.put_newline();
            adisp.put("ret=", None);
            adisp.put_single(put_type, f.ret.as_ref());
            adisp.indent_count -= 1;
        }
        ComptimeType::Void(t) => adisp.put_src(&t.pos),
        ComptimeType::FolderOrFile(t) => adisp.put_src(&t.pos),
        ComptimeType::Tuple(t) => {
            adisp.put_src(&t.pos);
            adisp.put_list(put_type, &t.children);
        }
        _ => {
            adisp.put(" %%TODO%%", None);
            adisp.put_src(ty.pos());
        }
    }
}

fn put_ast_node(adisp: &mut Adisp, node: &SyntaxNode) {
    adisp.put(syntax_node_kind(node), Some(colors::CYAN));
    match node {
        SyntaxNode::Block(b) => {
            adisp.put(&format!(" {}", bracket_tag_str(b.tag)), None);
            adisp.put_src(&b.pos);
            adisp.put_list(put_ast_node, &b.items);
        }
        SyntaxNode::BinaryExpression(b) => {
            adisp.put(&format!(" {}", op_tag_str(b.tag)), None);
            adisp.put_src(&b.pos);
            adisp.put_list(put_ast_node, &b.items);
        }
        SyntaxNode::Operator(o) => {
            adisp.put(&format!(" {}", json_string(&o.op)), Some(colors::YELLOW));
            adisp.put_src(&o.pos);
        }
        SyntaxNode::OperatorSegment(seg) => {
            adisp.put_src(&seg.pos);
            adisp.put_list(put_ast_node, &seg.items);
        }
        SyntaxNode::Whitespace(w) => {
            let s = if w.nl { "\n" } else { " " };
            adisp.put(&format!(" {}", json_string(s)), None);
            adisp.put_src(&w.pos);
        }
        SyntaxNode::Identifier(id) => {
            adisp.put(&format!(" {}", ident_tag_str(id.ident_tag)), None);
            let display = if is_simple_ident(&id.str) {
                id.str.clone()
            } else {
                format!("#{}", json_string(&id.str))
            };
            adisp.put(&format!(" {display}"), Some(colors::BLUE));
            adisp.put_src(&id.pos);
        }
        SyntaxNode::StrSeg(s) => {
            adisp.put(&format!(" {}", json_string(&s.str)), Some(colors::GREEN));
            adisp.put_src(&s.pos);
        }
        SyntaxNode::Raw(r) => {
            adisp.put(&format!(" {}", raw_tag_str(r.tag)), None);
            adisp.put_src(&r.pos);
        }
        SyntaxNode::Err(e) => {
            adisp.put(" %%TODO%%", None);
            adisp.put_src(&e.pos);
        }
    }
}

pub fn dump_ast_node_list(nodes: &[SyntaxNode], depth: usize) -> String {
    let mut adisp = Adisp::new(depth);
    adisp.put_list(put_ast_node, nodes);
    adisp.end()
}

pub fn dump_ast_node(node: &SyntaxNode, depth: usize) -> String {
    let mut adisp = Adisp::new(depth);
    adisp.put_single(put_ast_node, node);
    adisp.end()
}

pub fn dump_type(ty: &ComptimeType, depth: usize) -> String {
    let mut adisp = Adisp::new(depth);
    adisp.put_single(put_type, ty);
    adisp.end()
}

pub fn dump_destructure(d: &Destructure, depth: usize) -> String {
    let mut adisp = Adisp::new(depth);
    adisp.put_multi(put_destructure, d);
    adisp.end()
}

pub fn dump_block(block: &AnalysisBlock, depth: usize) -> String {
    let mut adisp = Adisp::new(depth);
    adisp.put_multi(put_block, block);
    adisp.end()
}
