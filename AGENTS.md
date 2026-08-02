BE3 project

Functionality:
- When making changes to serialization formats or network requests, do not consider backwards compatibility with existing clients or data.

UI style:
- Never use unicode characters for icons. Always prefer icon libraries / plain text.

Code style:
- Prefer a.rs over a/mod.rs.

Tests:
- Keep test files seperate from code files.
- Give every test its own seperate file named the same as the function inside it. Tests for `src/a.rs` go in `src/a/tests/fn_name_1.rs`; test imports and support functions go in `src/a/tests.rs`. Tests for a crate root such as `src/lib.rs` instead go in `src/tests/fn_name_1.rs`, with imports and support functions in `src/tests.rs`. Import test modules with plain `mod tests;` and plain child `mod fn_name_1;` declarations; do not use `#[path]`. Production files only import their `tests.rs` module and do not define every individual test.
- Do not add tests for GUI features.
- Do not add irrelevant or useless tests. If a change needs manual testing, note what needs testing in your final output.

Verification:
- After making changes, always run: `cargo fmt` and `cargo nextest run`. This should take less than 2 minutes including compilation time. Then, commit changes to git. Do not push.
- If working on multiple changes, commit to git after each one.
- Do not perform any manual verification besides those commands.
