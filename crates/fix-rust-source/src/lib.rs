use std::error::Error;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Parser, Tree};

pub fn fix_repository(root: &Path, check: bool) -> Result<(), Box<dyn Error>> {
    let mut violations = find_layout_violations(root)?;
    violations.extend(find_files_with_comments(root)?);
    violations.sort_unstable();
    violations.dedup();

    if check {
        if violations.is_empty() {
            return Ok(());
        }
        return Err(format!("Rust source fixes required:\n{}", violations.join("\n")).into());
    }

    rename_module_files(root)?;
    strip_repository_comments(root)?;
    fix_test_layout(root)?;

    let remaining = find_layout_violations(root)?;
    if !remaining.is_empty() {
        return Err(format!(
            "Rust source layout could not be fixed:\n{}",
            remaining.join("\n")
        )
        .into());
    }
    Ok(())
}

pub fn strip_comments(source: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let tree = parse(source)?;
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

fn parse(source: &[u8]) -> Result<Tree, Box<dyn Error>> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
    let tree = parser
        .parse(source, None)
        .ok_or("Rust parser did not produce a syntax tree")?;
    if tree.root_node().has_error() {
        return Err("Rust parser found invalid syntax".into());
    }
    Ok(tree)
}

struct Comment {
    range: Range<usize>,
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

#[derive(Clone)]
struct TestFunction {
    name: String,
    range: Range<usize>,
}

struct InlineTests {
    range: Range<usize>,
    body_range: Range<usize>,
    functions: Vec<TestFunction>,
}

struct PathModule {
    name: String,
    range: Range<usize>,
    custom_path: String,
    retained_attributes: Vec<String>,
}

fn path_modules(node: Node<'_>, source: &[u8]) -> Vec<PathModule> {
    let mut modules = Vec::new();
    let mut attributes = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "attribute_item" {
            attributes.push(child);
            continue;
        }
        if child.kind() == "mod_item" && child.child_by_field_name("body").is_none() {
            let path_attribute = attributes.iter().find_map(|attribute| {
                let attribute = text(source, attribute.byte_range());
                parse_path_attribute(attribute).map(|path| (attribute, path))
            });
            if let (Some(name), Some((path_attribute, custom_path))) =
                (child.child_by_field_name("name"), path_attribute)
            {
                modules.push(PathModule {
                    name: text(source, name.byte_range()).to_owned(),
                    range: attributes
                        .first()
                        .map_or(child.start_byte(), Node::start_byte)
                        ..child.end_byte(),
                    custom_path,
                    retained_attributes: attributes
                        .iter()
                        .map(|attribute| text(source, attribute.byte_range()))
                        .filter(|attribute| *attribute != path_attribute)
                        .map(str::to_owned)
                        .collect(),
                });
            }
        }
        attributes.clear();
    }
    modules
}

fn parse_path_attribute(attribute: &str) -> Option<String> {
    let compact = attribute
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    if !compact.starts_with("#[path=") {
        return None;
    }
    let start = attribute.find('"')? + 1;
    let end = attribute[start..].find('"')? + start;
    Some(attribute[start..end].to_owned())
}

fn attributed_test_functions(node: Node<'_>, source: &[u8]) -> Vec<TestFunction> {
    let mut functions = Vec::new();
    let mut attributes = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "attribute_item" {
            attributes.push(child);
            continue;
        }
        if child.kind() == "function_item"
            && attributes
                .iter()
                .any(|attribute| is_test_attribute(&source[attribute.byte_range()]))
        {
            if let Some(name) = child.child_by_field_name("name") {
                functions.push(TestFunction {
                    name: text(source, name.byte_range())
                        .trim_start_matches("r#")
                        .to_owned(),
                    range: attributes.first().unwrap().start_byte()..child.end_byte(),
                });
            }
        }
        attributes.clear();
    }
    functions
}

fn is_test_attribute(attribute: &[u8]) -> bool {
    let compact = String::from_utf8_lossy(attribute)
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    compact == "#[test]" || compact.ends_with("::test]")
}

fn inline_tests(node: Node<'_>, source: &[u8]) -> Option<InlineTests> {
    let mut attributes = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "attribute_item" {
            attributes.push(child);
            continue;
        }
        if child.kind() == "mod_item"
            && child
                .child_by_field_name("name")
                .is_some_and(|name| text(source, name.byte_range()) == "tests")
        {
            if let Some(body) = child.child_by_field_name("body") {
                return Some(InlineTests {
                    range: attributes
                        .first()
                        .map_or(child.start_byte(), Node::start_byte)
                        ..child.end_byte(),
                    body_range: body.start_byte() + 1..body.end_byte() - 1,
                    functions: attributed_test_functions(body, source),
                });
            }
        }
        attributes.clear();
    }
    None
}

fn text(source: &[u8], range: Range<usize>) -> &str {
    std::str::from_utf8(&source[range]).expect("Rust source is UTF-8")
}

fn find_files_with_comments(root: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut paths = Vec::new();
    collect_rust_files(root, &mut paths)?;
    let mut violations = Vec::new();
    for path in paths {
        let source = fs::read(&path)?;
        if strip_comments(&source)? != source {
            violations.push(format!("comments: {}", relative(root, &path)));
        }
    }
    Ok(violations)
}

fn strip_repository_comments(root: &Path) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    collect_rust_files(root, &mut paths)?;
    for path in paths {
        let source = fs::read(&path)?;
        let stripped = strip_comments(&source)?;
        if stripped != source {
            fs::write(path, stripped)?;
        }
    }
    Ok(())
}

fn find_layout_violations(root: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut paths = Vec::new();
    collect_rust_files(root, &mut paths)?;
    let mut violations = Vec::new();

    for path in &paths {
        if path.file_name().is_some_and(|name| name == "mod.rs") {
            violations.push(format!("module file: {}", relative(root, path)));
        }

        let source = fs::read(path)?;
        let tree = parse(&source)?;
        if inline_tests(tree.root_node(), &source).is_some() {
            violations.push(format!("inline tests: {}", relative(root, path)));
        }
        if path_modules(tree.root_node(), &source)
            .iter()
            .any(|module| is_test_module_owner(path, &module.name))
        {
            violations.push(format!("test path attribute: {}", relative(root, path)));
        }

        let functions = attributed_test_functions(tree.root_node(), &source);
        if !functions.is_empty()
            && !(is_individual_test_file(path)
                && functions.len() == 1
                && path
                    .file_stem()
                    .is_some_and(|stem| stem == functions[0].name.as_str()))
        {
            violations.push(format!("test functions: {}", relative(root, path)));
        }
    }

    for path in &paths {
        if !is_individual_test_file(path) {
            continue;
        }
        let aggregator = path.parent().unwrap().with_extension("rs");
        let module = path.file_stem().unwrap().to_string_lossy();
        if !aggregator_declares(&aggregator, &module)? {
            violations.push(format!(
                "test module declaration: {}",
                relative(root, &aggregator)
            ));
        }
        if let Some((production, module)) = production_and_module_for_tests(path.parent().unwrap())
        {
            if production.exists() && !aggregator_declares(&production, module)? {
                violations.push(format!(
                    "tests declaration: {}",
                    relative(root, &production)
                ));
            }
        }
    }

    let mut test_directories = paths
        .iter()
        .filter(|path| is_individual_test_file(path))
        .map(|path| path.parent().unwrap().to_path_buf())
        .collect::<Vec<_>>();
    test_directories.sort_unstable();
    test_directories.dedup();
    for directory in test_directories {
        let aggregator = directory.with_extension("rs");
        if !aggregator.exists() {
            continue;
        }
        for module in declared_modules(&fs::read_to_string(&aggregator)?) {
            if !directory.join(&module).with_extension("rs").exists() {
                violations.push(format!(
                    "stale test module: {}::{module}",
                    relative(root, &aggregator)
                ));
            }
        }
    }

    Ok(violations)
}

fn rename_module_files(root: &Path) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    collect_rust_files(root, &mut paths)?;
    paths.sort_unstable_by_key(|path| std::cmp::Reverse(path.components().count()));
    for path in paths {
        if !path.file_name().is_some_and(|name| name == "mod.rs") {
            continue;
        }
        let target = path
            .parent()
            .ok_or("mod.rs has no parent")?
            .with_extension("rs");
        if target.exists() {
            return Err(format!("cannot move {} over {}", path.display(), target.display()).into());
        }
        fs::rename(path, target)?;
    }
    Ok(())
}

fn fix_test_layout(root: &Path) -> Result<(), Box<dyn Error>> {
    fix_test_path_modules(root)?;

    let mut paths = Vec::new();
    collect_rust_files(root, &mut paths)?;
    paths.sort_unstable();
    for path in paths {
        fix_inline_tests(&path)?;
    }

    let mut paths = Vec::new();
    collect_rust_files(root, &mut paths)?;
    paths.sort_unstable();
    for path in paths {
        fix_top_level_tests(&path)?;
    }

    synchronize_test_modules(root)
}

fn fix_test_path_modules(root: &Path) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    collect_rust_files(root, &mut paths)?;
    paths.sort_unstable();
    for path in paths {
        if !path.exists() {
            continue;
        }
        let source = fs::read(&path)?;
        let tree = parse(&source)?;
        let modules = path_modules(tree.root_node(), &source)
            .into_iter()
            .filter(|module| is_test_module_owner(&path, &module.name))
            .collect::<Vec<_>>();
        if modules.is_empty() {
            continue;
        }
        let mut fixed = source;
        for module in modules.iter().rev() {
            let custom = path.parent().unwrap().join(&module.custom_path);
            let target = if module.name == "tests" {
                test_locations(&path)?.0
            } else {
                path.with_extension("")
                    .join(&module.name)
                    .with_extension("rs")
            };
            if custom != target {
                if target.exists() {
                    return Err(format!(
                        "cannot move {} over {}",
                        custom.display(),
                        target.display()
                    )
                    .into());
                }
                if !custom.exists() {
                    return Err(format!("test module does not exist: {}", custom.display()).into());
                }
                fs::create_dir_all(target.parent().unwrap())?;
                fs::rename(custom, target)?;
            }
            let mut replacement = module.retained_attributes.join("\n");
            if !replacement.is_empty() {
                replacement.push('\n');
            }
            replacement.push_str(&format!("mod {};", module.name));
            fixed.splice(module.range.clone(), replacement.bytes());
        }
        fs::write(path, fixed)?;
    }
    Ok(())
}

fn is_test_module_owner(path: &Path, module: &str) -> bool {
    module == "tests"
        || path.file_name().is_some_and(|name| name == "tests.rs")
        || path.file_name().is_some_and(|name| name == "main_tests.rs")
}

fn fix_inline_tests(path: &Path) -> Result<(), Box<dyn Error>> {
    let source = fs::read(path)?;
    let tree = parse(&source)?;
    let Some(inline) = inline_tests(tree.root_node(), &source) else {
        return Ok(());
    };
    let (aggregator, tests_directory) = test_locations(path)?;
    fs::create_dir_all(&tests_directory)?;

    let support = remove_ranges(
        &source[inline.body_range.clone()],
        &inline
            .functions
            .iter()
            .map(|function| {
                function.range.start - inline.body_range.start
                    ..function.range.end - inline.body_range.start
            })
            .collect::<Vec<_>>(),
    );
    merge_support(&aggregator, &support)?;
    for function in &inline.functions {
        write_test_file(&tests_directory, function, &source)?;
    }
    ensure_modules(&aggregator, &inline.functions)?;

    let mut fixed = remove_ranges(&source, std::slice::from_ref(&inline.range));
    append_test_declaration(&mut fixed, test_module_name(path));
    fs::write(path, fixed)?;
    Ok(())
}

fn fix_top_level_tests(path: &Path) -> Result<(), Box<dyn Error>> {
    let source = fs::read(path)?;
    let tree = parse(&source)?;
    let functions = attributed_test_functions(tree.root_node(), &source);
    if functions.is_empty()
        || (is_individual_test_file(path)
            && functions.len() == 1
            && path
                .file_stem()
                .is_some_and(|stem| stem == functions[0].name.as_str()))
    {
        return Ok(());
    }

    let (aggregator, tests_directory) = test_locations(path)?;
    fs::create_dir_all(&tests_directory)?;

    let fixed = remove_ranges(
        &source,
        &functions
            .iter()
            .map(|function| function.range.clone())
            .collect::<Vec<_>>(),
    );
    if is_individual_test_file(path) {
        merge_support(&aggregator, &fixed)?;
        fs::remove_file(path)?;
        for function in &functions {
            write_test_file(&tests_directory, function, &source)?;
        }
    } else {
        for function in &functions {
            write_test_file(&tests_directory, function, &source)?;
        }
        let mut fixed = fixed;
        if path != aggregator {
            append_test_declaration(&mut fixed, test_module_name(path));
        }
        fs::write(path, fixed)?;
    }
    ensure_modules(&aggregator, &functions)?;
    Ok(())
}

fn test_locations(path: &Path) -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
    if is_individual_test_file(path) {
        let tests_directory = path.parent().unwrap().to_path_buf();
        return Ok((tests_directory.with_extension("rs"), tests_directory));
    }
    if path.file_name().is_some_and(|name| name == "tests.rs") {
        return Ok((path.to_path_buf(), path.with_extension("")));
    }
    let stem = path.file_stem().ok_or("Rust file has no stem")?;
    let parent = path.parent().unwrap();
    let tests_directory = if stem == "lib" {
        parent.join("tests")
    } else if stem == "main" && parent.join("lib.rs").exists() {
        parent.join("main_tests")
    } else if stem == "main" {
        parent.join("tests")
    } else {
        parent.join(stem).join("tests")
    };
    Ok((tests_directory.with_extension("rs"), tests_directory))
}

fn is_individual_test_file(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .is_some_and(|name| matches!(name.to_str(), Some("tests" | "main_tests")))
        && !path.file_name().is_some_and(|name| name == "tests.rs")
}

fn write_test_file(
    directory: &Path,
    function: &TestFunction,
    source: &[u8],
) -> Result<(), Box<dyn Error>> {
    let path = directory.join(format!("{}.rs", function.name));
    let body = text(source, function.range.clone()).trim();
    let content = format!("use super::*;\n\n{body}\n");
    if path.exists() && fs::read_to_string(&path)? != content {
        return Err(format!("test file already exists: {}", path.display()).into());
    }
    fs::write(path, content)?;
    Ok(())
}

fn merge_support(aggregator: &Path, support: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut support = String::from_utf8(support.to_vec())?;
    let mut content = if aggregator.exists() {
        fs::read_to_string(aggregator)?
    } else {
        String::new()
    };
    if has_line(&content, "use super::*;") {
        support = support
            .lines()
            .filter(|line| line.trim() != "use super::*;")
            .collect::<Vec<_>>()
            .join("\n");
    }
    let support = support.trim();
    if support.is_empty() {
        if content.is_empty() {
            fs::write(aggregator, "use super::*;\n")?;
        }
        return Ok(());
    }
    if !content.contains(support) {
        if !content.is_empty() && !content.ends_with("\n\n") {
            content.push('\n');
        }
        content.push_str(support);
        content.push('\n');
        fs::write(aggregator, content)?;
    }
    Ok(())
}

fn ensure_modules(aggregator: &Path, functions: &[TestFunction]) -> Result<(), Box<dyn Error>> {
    let mut content = if aggregator.exists() {
        fs::read_to_string(aggregator)?
    } else {
        String::new()
    };
    for function in functions {
        let declaration = format!("mod {};", function.name);
        if !has_plain_module(&content, &function.name) {
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(&declaration);
            content.push('\n');
        }
    }
    fs::write(aggregator, content)?;
    Ok(())
}

fn append_test_declaration(source: &mut Vec<u8>, module: &str) {
    let content = String::from_utf8_lossy(source);
    if has_plain_module(&content, module) {
        return;
    }
    if !source.ends_with(b"\n") {
        source.push(b'\n');
    }
    source.extend_from_slice(format!("\n#[cfg(test)]\nmod {module};\n").as_bytes());
}

fn test_module_name(path: &Path) -> &str {
    if path.file_name().is_some_and(|name| name == "main.rs")
        && path
            .parent()
            .is_some_and(|parent| parent.join("lib.rs").exists())
    {
        "main_tests"
    } else {
        "tests"
    }
}

fn synchronize_test_modules(root: &Path) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    collect_rust_files(root, &mut paths)?;
    let test_files = paths
        .into_iter()
        .filter(|path| is_individual_test_file(path))
        .collect::<Vec<_>>();
    let mut directories = test_files
        .iter()
        .map(|path| path.parent().unwrap().to_path_buf())
        .collect::<Vec<_>>();
    directories.sort_unstable();
    directories.dedup();
    for directory in &directories {
        remove_stale_modules(&directory.with_extension("rs"), directory)?;
    }
    for path in test_files {
        let aggregator = path.parent().unwrap().with_extension("rs");
        let module = path.file_stem().unwrap().to_string_lossy().into_owned();
        ensure_modules(
            &aggregator,
            &[TestFunction {
                name: module,
                range: 0..0,
            }],
        )?;
        if let Some((production, production_module)) =
            production_and_module_for_tests(path.parent().unwrap())
        {
            if production.exists() {
                let mut source = fs::read(&production)?;
                append_test_declaration(&mut source, production_module);
                fs::write(production, source)?;
            }
        }
    }
    Ok(())
}

fn remove_stale_modules(aggregator: &Path, directory: &Path) -> Result<(), Box<dyn Error>> {
    if !aggregator.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(aggregator)?;
    let mut kept = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        let stale = trimmed
            .strip_prefix("mod ")
            .and_then(|module| module.strip_suffix(';'))
            .is_some_and(|module| !directory.join(module).with_extension("rs").exists());
        if stale {
            while kept
                .last()
                .is_some_and(|line: &&str| line.trim().is_empty())
            {
                kept.pop();
            }
            while kept
                .last()
                .is_some_and(|line: &&str| line.trim().starts_with("#["))
            {
                kept.pop();
            }
            continue;
        }
        kept.push(line);
    }
    fs::write(aggregator, format!("{}\n", kept.join("\n")))?;
    Ok(())
}

fn production_and_module_for_tests(tests_directory: &Path) -> Option<(PathBuf, &'static str)> {
    if tests_directory
        .file_name()
        .is_some_and(|name| name == "main_tests")
    {
        return Some((tests_directory.parent()?.join("main.rs"), "main_tests"));
    }
    let module_directory = tests_directory.parent()?;
    let module = module_directory.file_name()?;
    let nested = module_directory.parent()?.join(module).with_extension("rs");
    if nested.exists() {
        return Some((nested, "tests"));
    }
    let library = module_directory.join("lib.rs");
    if library.exists() {
        return Some((library, "tests"));
    }
    let binary = module_directory.join("main.rs");
    binary.exists().then_some((binary, "tests"))
}

fn aggregator_declares(path: &Path, module: &str) -> Result<bool, Box<dyn Error>> {
    if !path.exists() {
        return Ok(false);
    }
    Ok(has_plain_module(&fs::read_to_string(path)?, module))
}

fn has_plain_module(source: &str, module: &str) -> bool {
    source
        .lines()
        .any(|line| line.trim() == format!("mod {module};"))
}

fn has_line(source: &str, expected: &str) -> bool {
    source.lines().any(|line| line.trim() == expected)
}

fn declared_modules(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("mod ")
                .and_then(|module| module.strip_suffix(';'))
                .map(str::to_owned)
        })
        .collect()
}

fn remove_ranges(source: &[u8], ranges: &[Range<usize>]) -> Vec<u8> {
    let mut ranges = ranges.to_vec();
    ranges.sort_unstable_by_key(|range| range.start);
    let mut output = Vec::with_capacity(source.len());
    let mut cursor = 0;
    for range in ranges {
        output.extend_from_slice(&source[cursor..range.start]);
        cursor = range.end;
    }
    output.extend_from_slice(&source[cursor..]);
    output
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
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
