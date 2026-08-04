use std::ops::Range;
use tree_sitter_md::{MarkdownCursor, MarkdownTree};

use super::{SynHlColorScope, SynHlFontFamily, SynHlStyle, SynHlTextSize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownTableAlignment {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownTableRow {
    pub range: Range<usize>,
    pub cells: Vec<Range<usize>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownTable {
    pub rows: Vec<MarkdownTableRow>,
    pub alignments: Vec<MarkdownTableAlignment>,
}

pub(super) fn styles(tree: &MarkdownTree, len: usize) -> Vec<SynHlStyle> {
    let mut styles = vec![SynHlStyle::plain(SynHlColorScope::MarkdownPlainText); len];
    let mut cursor = tree.walk();
    style_node(&mut cursor, &mut styles);
    styles
}

pub(super) fn tables(tree: &MarkdownTree) -> Vec<MarkdownTable> {
    let mut tables = Vec::new();
    let mut cursor = tree.walk();
    collect_tables(&mut cursor, &mut tables);
    tables
}

fn collect_tables(cursor: &mut MarkdownCursor<'_>, tables: &mut Vec<MarkdownTable>) {
    if cursor.node().kind() == "pipe_table" {
        tables.push(table(cursor));
        return;
    }
    if cursor.goto_first_child() {
        loop {
            collect_tables(cursor, tables);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn table(cursor: &mut MarkdownCursor<'_>) -> MarkdownTable {
    let mut rows = Vec::new();
    let mut alignments = Vec::new();
    if cursor.goto_first_child() {
        loop {
            let kind = cursor.node().kind();
            if matches!(
                kind,
                "pipe_table_header" | "pipe_table_delimiter_row" | "pipe_table_row"
            ) {
                let delimiter = kind == "pipe_table_delimiter_row";
                let row = table_row(cursor, delimiter, &mut alignments);
                rows.push(row);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    MarkdownTable { rows, alignments }
}

fn table_row(
    cursor: &mut MarkdownCursor<'_>,
    delimiter: bool,
    alignments: &mut Vec<MarkdownTableAlignment>,
) -> MarkdownTableRow {
    let node = cursor.node();
    let range = node.start_byte()..node.end_byte();
    let mut cells = Vec::new();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if matches!(node.kind(), "pipe_table_cell" | "pipe_table_delimiter_cell") {
                cells.push(node.start_byte()..node.end_byte());
                if delimiter {
                    alignments.push(table_alignment(cursor));
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    MarkdownTableRow { range, cells }
}

fn table_alignment(cursor: &mut MarkdownCursor<'_>) -> MarkdownTableAlignment {
    let mut left = false;
    let mut right = false;
    if cursor.goto_first_child() {
        loop {
            match cursor.node().kind() {
                "pipe_table_align_left" => left = true,
                "pipe_table_align_right" => right = true,
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    match (left, right) {
        (true, true) => MarkdownTableAlignment::Center,
        (_, true) => MarkdownTableAlignment::Right,
        _ => MarkdownTableAlignment::Left,
    }
}

fn style_node(cursor: &mut MarkdownCursor<'_>, styles: &mut [SynHlStyle]) {
    let node = cursor.node();
    let range = node.start_byte().min(styles.len())..node.end_byte().min(styles.len());
    let kind = node.kind();
    let parent_kind = node.parent().map(|parent| parent.kind());

    let contextual_symbol = match kind {
        "[" | "]" => parent_kind.is_some_and(|parent| {
            matches!(
                parent,
                "image"
                    | "inline_link"
                    | "shortcut_link"
                    | "collapsed_reference_link"
                    | "full_reference_link"
            )
        }),
        "(" | ")" => parent_kind.is_some_and(|parent| matches!(parent, "image" | "inline_link")),
        "!" => parent_kind == Some("image"),
        "|" => parent_kind.is_some_and(|parent| parent.starts_with("pipe_table")),
        "~" => parent_kind == Some("strikethrough"),
        _ => false,
    };
    if contextual_symbol {
        for style in &mut styles[range.clone()] {
            set_symbol(style);
        }
    }

    match kind {
        "strong_emphasis" => {
            for style in &mut styles[range.clone()] {
                style.bold = true;
            }
        }
        "emphasis" => {
            for style in &mut styles[range.clone()] {
                style.italic = true;
            }
        }
        "strikethrough" => {
            for style in &mut styles[range.clone()] {
                style.strikethrough = true;
            }
        }
        "atx_heading" => {
            let level = node
                .child(0)
                .and_then(|marker| marker.kind().strip_prefix("atx_h"))
                .and_then(|value| value.strip_suffix("_marker"))
                .and_then(|value| value.parse().ok())
                .unwrap_or(6);
            set_heading(&mut styles[range.clone()], level);
        }
        "setext_heading" => {
            let mut child = node.walk();
            let level = node
                .children(&mut child)
                .find_map(|child| match child.kind() {
                    "setext_h1_underline" => Some(1),
                    "setext_h2_underline" => Some(2),
                    _ => None,
                })
                .unwrap_or(2);
            set_heading(&mut styles[range.clone()], level);
        }
        "block_quote" => {
            for style in &mut styles[range.clone()] {
                style.italic = true;
            }
        }
        "pipe_table_header" => {
            for style in &mut styles[range.clone()] {
                style.bold = true;
            }
        }
        "code_span" | "indented_code_block" | "code_fence_content" => {
            for style in &mut styles[range.clone()] {
                style.color = SynHlColorScope::MarkdownCode;
                style.family = SynHlFontFamily::Monospace;
                style.size = SynHlTextSize::Body;
                style.bold = false;
                style.italic = false;
            }
        }
        "info_string" => {
            for style in &mut styles[range.clone()] {
                style.color = SynHlColorScope::MarkdownLink;
                style.family = SynHlFontFamily::Monospace;
            }
        }
        "link_text" | "link_label" | "image_description" | "uri_autolink" | "email_autolink" => {
            for style in &mut styles[range.clone()] {
                style.color = SynHlColorScope::MarkdownLink;
                style.underline = true;
            }
        }
        "link_destination" | "link_title" => {
            for style in &mut styles[range.clone()] {
                style.color = SynHlColorScope::MarkdownLink;
                style.family = SynHlFontFamily::Monospace;
            }
        }
        "backslash_escape" => {
            if let Some(style) = styles.get_mut(range.start) {
                set_symbol(style);
            }
        }
        "emphasis_delimiter"
        | "code_span_delimiter"
        | "fenced_code_block_delimiter"
        | "block_quote_marker"
        | "block_continuation"
        | "list_marker_plus"
        | "list_marker_minus"
        | "list_marker_star"
        | "list_marker_dot"
        | "list_marker_parenthesis"
        | "task_list_marker_checked"
        | "task_list_marker_unchecked"
        | "thematic_break"
        | "setext_h1_underline"
        | "setext_h2_underline"
        | "pipe_table_delimiter_row"
        | "pipe_table_delimiter_cell"
        | "pipe_table_align_left"
        | "pipe_table_align_right"
        | "hard_line_break"
        | "atx_h1_marker"
        | "atx_h2_marker"
        | "atx_h3_marker"
        | "atx_h4_marker"
        | "atx_h5_marker"
        | "atx_h6_marker" => {
            for style in &mut styles[range.clone()] {
                set_symbol(style);
            }
        }
        _ => {}
    }

    if cursor.goto_first_child() {
        loop {
            style_node(cursor, styles);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn set_heading(styles: &mut [SynHlStyle], level: u8) {
    for style in styles {
        style.bold = true;
        style.size = SynHlTextSize::Heading(level.clamp(1, 6));
    }
}

fn set_symbol(style: &mut SynHlStyle) {
    *style = SynHlStyle::plain(SynHlColorScope::MarkdownSymbol);
}

pub(super) fn collect_chain(
    cursor: &mut MarkdownCursor<'_>,
    start: usize,
    end: usize,
    result: &mut Vec<(usize, usize)>,
) {
    let node = cursor.node();
    if node.start_byte() > start || node.end_byte() < end {
        return;
    }
    result.push((node.start_byte(), node.end_byte()));
    if cursor.goto_first_child() {
        loop {
            collect_chain(cursor, start, end, result);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}
