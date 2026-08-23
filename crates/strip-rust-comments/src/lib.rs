use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Parser};

pub fn strip_repository(root: &Path, check: bool) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    collect_rust_files(root, &mut paths)?;
    paths.sort_unstable();

    let mut files_with_comments = Vec::new();
    for path in paths {
        let source = fs::read(&path)?;
        let stripped = strip_comments(&source)?;
        if stripped == source {
            continue;
        }
        if check {
            files_with_comments.push(path);
        } else {
            fs::write(path, stripped)?;
        }
    }

    if files_with_comments.is_empty() {
        return Ok(());
    }

    let paths = files_with_comments
        .iter()
        .map(|path| {
            path.strip_prefix(root)
                .unwrap_or(path)
                .display()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    Err(format!("Rust comments found in:\n{paths}").into())
}

pub fn strip_comments(source: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
    let tree = parser
        .parse(source, None)
        .ok_or("Rust parser did not produce a syntax tree")?;
    if tree.root_node().has_error() {
        return Err("Rust parser found invalid syntax".into());
    }

    let mut comments = Vec::new();
    collect_comments(tree.root_node(), &mut comments);
    comments.sort_unstable_by_key(|comment| comment.range.start);

    let mut stripped = Vec::with_capacity(source.len());
    let mut cursor = 0;
    for comment in comments {
        let line_start = source[..comment.range.start]
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |index| index + 1);
        let line_end = source[comment.range.end..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(source.len(), |index| comment.range.end + index + 1);
        let starts_line = source[line_start..comment.range.start]
            .iter()
            .all(|byte| byte.is_ascii_whitespace());
        let ends_line = source[comment.range.end..line_end]
            .iter()
            .all(|byte| byte.is_ascii_whitespace());

        let (start, end) = if starts_line && ends_line {
            (line_start, line_end)
        } else {
            let start = source[cursor..comment.range.start]
                .iter()
                .rposition(|byte| !matches!(*byte, b' ' | b'\t'))
                .map_or(cursor, |index| cursor + index + 1);
            (start, comment.range.end)
        };
        stripped.extend_from_slice(&source[cursor..start]);
        if comment.block && !(starts_line && ends_line) {
            let line_breaks = source[comment.range.clone()]
                .iter()
                .copied()
                .filter(|byte| matches!(byte, b'\n' | b'\r'))
                .collect::<Vec<_>>();
            if line_breaks.is_empty() {
                stripped.push(b' ');
            } else {
                stripped.extend(line_breaks);
            }
        }
        cursor = end;
    }
    stripped.extend_from_slice(&source[cursor..]);
    Ok(stripped)
}

struct Comment {
    range: std::ops::Range<usize>,
    block: bool,
}

fn collect_comments(node: Node<'_>, comments: &mut Vec<Comment>) {
    if matches!(node.kind(), "line_comment" | "block_comment") {
        comments.push(Comment {
            range: node.byte_range(),
            block: node.kind() == "block_comment",
        });
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_comments(child, comments);
    }
}

fn collect_rust_files(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if matches!(entry.file_name().to_str(), Some(".git" | "target")) {
                continue;
            }
            collect_rust_files(&path, paths)?;
        } else if file_type.is_file() && path.extension().is_some_and(|extension| extension == "rs")
        {
            paths.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
