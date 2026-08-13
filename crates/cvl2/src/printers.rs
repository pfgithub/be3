use std::collections::HashMap;

use crate::compiler::{
    syntax_node_kind, AnalysisBlock, AnalysisLine, ComptimeValue, ComptimeValueBuildArtifact,
    ComptimeValueKey, Destructure, DestructureExtract, RuntimeValue,
};
use crate::ct::Type;
use crate::parser::{
    colors, highlights, BracketTag, IdentifierTag, OpTag, RawTag, SyntaxNode, TokenPosition,
};

pub const UNLIMITED_DEPTH: usize = usize::MAX;

pub struct Adisp {
    indent_str: String,
    indent_count: usize,
    depth: usize,
    res: Vec<String>,
    cache: HashMap<usize, HashMap<usize, u64>>,
    reference_count: u64,
}

impl Adisp {
    pub fn new(depth: usize) -> Self {
        Adisp {
            indent_str: "\u{2502} ".to_string(),
            indent_count: 0,
            depth,
            res: Vec::new(),
            cache: HashMap::new(),
            reference_count: 0,
        }
    }

    pub fn end(self) -> String {
        self.res.concat()
    }

    pub fn with_indent<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.indent_count += 1;
        let result = f(self);
        self.indent_count -= 1;
        result
    }

    fn get_or_add_cache<T>(&mut self, printer_id: usize, value: &T) -> Option<u64> {
        let value_ptr = value as *const T as usize;
        let inner = self.cache.entry(printer_id).or_default();
        if let Some(existing) = inner.get(&value_ptr) {
            return Some(*existing);
        }
        let id = self.reference_count;
        self.reference_count += 1;
        inner.insert(value_ptr, id);
        None
    }

    pub fn put(&mut self, msg: &str, color: Option<&str>) {
        if let Some(c) = color {
            self.res.push(c.to_string());
        }
        self.res.push(msg.to_string());
        if color.is_some() {
            self.res.push(colors::RESET.to_string());
        }
    }

    pub fn put_with_nl(&mut self, msg: &str, color: Option<&str>) {
        for (i, seg) in msg.split('\n').enumerate() {
            if i > 0 {
                self.put_newline();
            }
            self.put(seg, color);
        }
    }

    pub fn put_newline(&mut self) {
        self.put("\n", None);
        let indent = self.indent_str.repeat(self.indent_count);
        self.put(&indent, Some(colors::BRBLACK));
    }

    pub fn put_src(&mut self, pos: &TokenPosition) {
        let msg = format!(" \u{b7} {}:{}:{}", pos.fyl, pos.lyn, pos.col);
        self.put(&msg, Some(colors::BRBLACK));
    }

    pub fn put_check_depth(&mut self, n: Option<usize>) -> bool {
        if self.indent_count > self.depth {
            self.put_newline();
            self.put("...", Some(colors::BRBLACK));
            if let Some(n) = n {
                if n > 0 {
                    let msg = format!(" {n} item{}", if n == 1 { "" } else { "s" });
                    self.put(&msg, Some(colors::BRBLACK));
                }
            }
            return true;
        }
        false
    }

    pub fn put_inline<T>(&mut self, printer: &SinglePrinter<T>, value: &T) {
        let cached = self.get_or_add_cache(printer.printer_id(), value);
        if let Some(id) = cached {
            self.put(&format!("[referenced value {id}]"), None);
        }
        (printer.print)(self, value);
    }

    pub fn put_single<T>(&mut self, printer: &SinglePrinter<T>, value: &T) {
        self.with_indent(|adisp| {
            if adisp.put_check_depth(None) {
                return;
            }
            adisp.put_newline();
            adisp.put_inline(printer, value);
        });
    }

    pub fn put_multi<T>(&mut self, printer: &MultiPrinter<T>, value: &T) {
        self.with_indent(|adisp| {
            if adisp.put_check_depth(None) {
                return;
            }
            (printer.print)(adisp, value);
        });
    }

    pub fn put_list<T>(&mut self, printer: &SinglePrinter<T>, children: &[T]) {
        self.with_indent(|adisp| {
            if children.is_empty() {
                adisp.put_newline();
                adisp.put("*no children*", Some(colors::BRBLACK));
                return;
            }
            if adisp.put_check_depth(Some(children.len())) {
                return;
            }
            for child in children {
                adisp.put_newline();
                adisp.put_inline(printer, child);
            }
        });
    }
}

pub struct SinglePrinter<T> {
    print: fn(&mut Adisp, &T),
}

impl<T> SinglePrinter<T> {
    pub const fn new(print: fn(&mut Adisp, &T)) -> Self {
        SinglePrinter { print }
    }

    fn printer_id(&self) -> usize {
        self.print as usize
    }

    pub fn dump(&self, value: &T, depth: usize) -> String {
        let mut adisp = Adisp::new(depth);
        adisp.put_single(self, value);
        adisp.end()
    }

    pub fn dump_list(&self, values: &[T], depth: usize) -> String {
        let mut adisp = Adisp::new(depth);
        adisp.put_list(self, values);
        adisp.end()
    }
}

pub struct MultiPrinter<T> {
    print: fn(&mut Adisp, &T),
}

impl<T> MultiPrinter<T> {
    pub const fn new(print: fn(&mut Adisp, &T)) -> Self {
        MultiPrinter { print }
    }

    pub fn dump(&self, value: &T, depth: usize) -> String {
        let mut adisp = Adisp::new(depth);
        adisp.put_multi(self, value);
        adisp.end()
    }
}

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
        BracketTag::InlineComment => "inline_comment",
        BracketTag::None => "",
    }
}

fn ident_tag_str(tag: IdentifierTag) -> &'static str {
    match tag {
        IdentifierTag::Normal => "normal",
        IdentifierTag::Access => "access",
        IdentifierTag::Builtin => "builtin",
        IdentifierTag::Number => "number",
        IdentifierTag::Discard => "discard",
    }
}

fn raw_tag_str(tag: RawTag) -> &'static str {
    match tag {
        RawTag::Return => "return",
        RawTag::Discard => "discard",
        RawTag::Void => "void",
        RawTag::String => "string",
        RawTag::Comment => "comment",
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

fn analysis_line_tag(line: &AnalysisLine) -> &'static str {
    match line {
        AnalysisLine::ComptimeKvListInit { .. } => "comptime:kv_list_init",
        AnalysisLine::ComptimeKvListAppend { .. } => "comptime:kv_list_append",
        AnalysisLine::Call { .. } => "call",
        AnalysisLine::Break { .. } => "break",
        AnalysisLine::Args { .. } => "args",
        AnalysisLine::ComptimeFileCreate { .. } => "comptime:file_create",
        AnalysisLine::McExecRaw { .. } => "mc:exec_raw",
    }
}

pub(crate) fn analysis_line_pos(line: &AnalysisLine) -> &TokenPosition {
    match line {
        AnalysisLine::ComptimeKvListInit { pos } => pos,
        AnalysisLine::ComptimeKvListAppend { pos, .. } => pos,
        AnalysisLine::Call { pos, .. } => pos,
        AnalysisLine::Break { pos, .. } => pos,
        AnalysisLine::Args { pos } => pos,
        AnalysisLine::ComptimeFileCreate { pos, .. } => pos,
        AnalysisLine::McExecRaw { pos, .. } => pos,
    }
}

fn comptime_value_kind(value: &ComptimeValue) -> &'static str {
    match value {
        ComptimeValue::Key(_) => "key",
        ComptimeValue::Namespace(_) => "namespace",
        ComptimeValue::Type(_) => "type",
        ComptimeValue::Ast(_) => "ast",
        ComptimeValue::Void(_) => "void",
        ComptimeValue::KvFields(_) => "comptime:kv_fields",
        ComptimeValue::Fn(_) => "fn",
        ComptimeValue::Optional(_) => "optional",
        ComptimeValue::BuildArtifact(_) => "build_artifact",
        ComptimeValue::Uint8Array(_) => "uint8array",
        ComptimeValue::ExportList(_) => "export_list",
        ComptimeValue::CExportName(_) => "c:export_name",
        ComptimeValue::McIdentifier(_) => "mc:identifier",
        ComptimeValue::McResult(_) => "mc:result",
        ComptimeValue::McNbtRef(_) => "mc:nbt_ref",
        ComptimeValue::Error(_) => "error",
        ComptimeValue::Mc(_) => "mc",
    }
}

fn runtime_value_kind(value: &RuntimeValue) -> &'static str {
    match value {
        RuntimeValue::Runtime(_) => "runtime",
        RuntimeValue::Comptime(c) => comptime_value_kind(c),
    }
}

fn destructure_extract_kind(extract: &DestructureExtract) -> &'static str {
    match extract {
        DestructureExtract::SingleItem { .. } => "single_item",
        DestructureExtract::List { .. } => "list",
        DestructureExtract::Map { .. } => "map",
        DestructureExtract::Discard { .. } => "discard",
    }
}

fn destructure_extract_pos(extract: &DestructureExtract) -> &TokenPosition {
    match extract {
        DestructureExtract::SingleItem { pos, .. } => pos,
        DestructureExtract::List { pos, .. } => pos,
        DestructureExtract::Map { pos, .. } => pos,
        DestructureExtract::Discard { pos } => pos,
    }
}

fn print_block(adisp: &mut Adisp, block: &AnalysisBlock) {
    if adisp.put_check_depth(None) {
        return;
    }
    for (i, expr) in block.lines.iter().enumerate() {
        adisp.put_newline();
        adisp.put(&format!("{i} = "), None);
        adisp.put(analysis_line_tag(expr), Some(colors::MAGENTA));
        match expr {
            AnalysisLine::Call { pos, method, arg } => {
                adisp.put_src(pos);
                adisp.with_indent(|adisp| {
                    adisp.put_newline();
                    adisp.put("method: ", None);
                    adisp.put_inline(&printers::RUNTIME_VALUE, method);
                    adisp.put_newline();
                    adisp.put("arg: ", None);
                    adisp.put_inline(&printers::RUNTIME_VALUE, arg);
                });
            }
            AnalysisLine::ComptimeKvListInit { pos } => {
                adisp.put_src(pos);
            }
            AnalysisLine::ComptimeKvListAppend {
                pos,
                key,
                list,
                value,
            } => {
                adisp.put_src(pos);
                adisp.with_indent(|adisp| {
                    adisp.put_newline();
                    adisp.put("key: ", None);
                    adisp.put_inline(&printers::RUNTIME_VALUE, key);
                    adisp.put_newline();
                    adisp.put("list: ", None);
                    adisp.put_inline(&printers::RUNTIME_VALUE, list);
                    adisp.put_newline();
                    adisp.put("value: ", None);
                    adisp.put_inline(&printers::RUNTIME_VALUE, value);
                });
            }
            AnalysisLine::Args { pos } => {
                adisp.put_src(pos);
            }
            AnalysisLine::McExecRaw { pos, command } => {
                adisp.put_src(pos);
                adisp.with_indent(|adisp| {
                    adisp.put_newline();
                    adisp.put("command: ", None);
                    adisp.put_inline(&printers::RUNTIME_VALUE, command);
                });
            }
            _ => {
                adisp.put(" %%TODO%%", None);
                adisp.put_src(analysis_line_pos(expr));
            }
        }
    }
}

fn print_runtime_value(adisp: &mut Adisp, rtv: &RuntimeValue) {
    adisp.put(runtime_value_kind(rtv), Some(colors::GREEN));
    match rtv {
        RuntimeValue::Comptime(ComptimeValue::Key(key)) => match key {
            ComptimeValueKey::String { key } => {
                adisp.put(" string", Some(colors::GREEN));
                adisp.put(&json_string(key), None);
            }
            ComptimeValueKey::Symbol { key, child } => {
                adisp.put(" symbol", Some(colors::GREEN));
                adisp.put(&format!(" {key:?}"), None);
                adisp.with_indent(|adisp| {
                    adisp.put_newline();
                    adisp.put("child: ", None);
                    adisp.put_inline(&printers::TYPE, child);
                });
            }
        },
        RuntimeValue::Runtime(idx) => {
            adisp.put(&format!(" {}", idx.0), None);
        }
        RuntimeValue::Comptime(ComptimeValue::Ast(ast)) => {
            adisp.put_list(&printers::AST_NODE, &ast.ast);
        }
        RuntimeValue::Comptime(ComptimeValue::Void(_)) => {}
        RuntimeValue::Comptime(ComptimeValue::Fn(_)) => {}
        RuntimeValue::Comptime(ComptimeValue::McIdentifier(mc)) => {
            adisp.put(&format!(" {}", mc.namespace), Some(colors::CYAN));
            adisp.put(":", None);
            adisp.put(&mc.path, Some(colors::BLUE));
        }
        _ => {
            adisp.put(" %%TODO%%", None);
        }
    }
}

fn print_destructure(adisp: &mut Adisp, destructure: &Destructure) {
    adisp.put_newline();
    adisp.put("extract: ", None);
    adisp.put_inline(&printers::DESTRUCTURE_EXTRACT, &destructure.extract);
    adisp.put_newline();
    adisp.put("type: ", None);
    adisp.put_inline(&printers::TYPE, &destructure.ty);
}

fn print_destructure_extract(adisp: &mut Adisp, extract: &DestructureExtract) {
    adisp.put(destructure_extract_kind(extract), Some(colors::CYAN));
    match extract {
        DestructureExtract::SingleItem { target, pos } => {
            adisp.put(&format!(" {target}"), Some(colors::GREEN));
            adisp.put_src(pos);
        }
        DestructureExtract::List { items, pos } => {
            adisp.put_src(pos);
            adisp.put_list(&printers::DESTRUCTURE_EXTRACT, items);
        }
        _ => {
            adisp.put(" %%TODO%%", None);
            adisp.put_src(destructure_extract_pos(extract));
        }
    }
}

fn print_type(adisp: &mut Adisp, ty: &Type) {
    adisp.put(&ty.dump(), Some(colors::YELLOW));
}

fn print_ast_node(adisp: &mut Adisp, entity: &SyntaxNode) {
    adisp.put(syntax_node_kind(entity), Some(colors::CYAN));
    match entity {
        SyntaxNode::Block(b) => {
            adisp.put(&format!(" {}", bracket_tag_str(b.tag)), None);
            adisp.put_src(&b.pos);
            adisp.put_list(&printers::AST_NODE, &b.items);
        }
        SyntaxNode::BinaryExpression(b) => {
            adisp.put(&format!(" {}", op_tag_str(b.tag)), None);
            adisp.put_src(&b.pos);
            adisp.put_list(&printers::AST_NODE, &b.items);
        }
        SyntaxNode::Operator(o) => {
            adisp.put(&format!(" {}", json_string(&o.op)), Some(colors::YELLOW));
            adisp.put_src(&o.pos);
        }
        SyntaxNode::OperatorSegment(seg) => {
            adisp.put_src(&seg.pos);
            adisp.put_list(&printers::AST_NODE, &seg.items);
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
        SyntaxNode::Raw(r) => {
            adisp.put(&format!(" {}", raw_tag_str(r.tag)), None);
            adisp.put(
                &format!(" {}", json_string(&r.raw)),
                Some(highlights::STRING),
            );
            adisp.put_src(&r.pos);
        }
        _ => {
            adisp.put(" %%TODO%%", None);
            adisp.put_src(syntax_node_pos(entity));
        }
    }
}

fn print_folder_or_file(adisp: &mut Adisp, entity: &ComptimeValueBuildArtifact) {
    match entity {
        ComptimeValueBuildArtifact::File(file) => {
            adisp.put("File", Some(colors::BLUE));
            adisp.with_indent(|adisp| {
                adisp.put_newline();
                adisp.put_with_nl(&String::from_utf8_lossy(&file.value), Some(colors::WHITE));
            });
        }
        ComptimeValueBuildArtifact::Folder(folder) => {
            adisp.put("Folder", Some(colors::BLUE));
            adisp.with_indent(|adisp| {
                for (key, value) in &folder.value {
                    adisp.put_newline();
                    adisp.put(&json_string(key), Some(colors::GREEN));
                    adisp.put(" = ", None);
                    adisp.put_inline(&printers::FOLDER_OR_FILE, value);
                }
                if folder.value.is_empty() {
                    adisp.put_newline();
                    adisp.put("*empty folder*", Some(colors::BRBLACK));
                }
            });
        }
    }
}

#[allow(clippy::module_inception)]
pub mod printers {
    use super::{
        print_ast_node, print_block, print_destructure, print_destructure_extract,
        print_folder_or_file, print_runtime_value, print_type, MultiPrinter, SinglePrinter,
    };
    use crate::compiler::{
        AnalysisBlock, ComptimeValueBuildArtifact, Destructure, DestructureExtract, RuntimeValue,
    };
    use crate::ct::Type;
    use crate::parser::SyntaxNode;

    pub static BLOCK: MultiPrinter<AnalysisBlock> = MultiPrinter::new(print_block);
    pub static RUNTIME_VALUE: SinglePrinter<RuntimeValue> = SinglePrinter::new(print_runtime_value);
    pub static DESTRUCTURE: MultiPrinter<Destructure> = MultiPrinter::new(print_destructure);
    pub static DESTRUCTURE_EXTRACT: SinglePrinter<DestructureExtract> =
        SinglePrinter::new(print_destructure_extract);
    pub static TYPE: SinglePrinter<Type> = SinglePrinter::new(print_type);
    pub static AST_NODE: SinglePrinter<SyntaxNode> = SinglePrinter::new(print_ast_node);
    pub static FOLDER_OR_FILE: SinglePrinter<ComptimeValueBuildArtifact> =
        SinglePrinter::new(print_folder_or_file);
}
