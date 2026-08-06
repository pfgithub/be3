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
- After making changes, always run: on Windows, `powershell -File ./scripts/verify.ps1`; on other OSes, `./scripts/verify.sh`. It runs clippy (applying the fixes it can), rustfmt and the tests, and fails if any clippy warning is left. This should take less than 2 minutes including compilation time. Then, commit changes to git. Do not push.
- Commit using `git add --all` and `git commit`. Don't check the status. Don't worry about it if the wrong file ends up in a commit unless it is supposed to be gitignored.
- When there are multiple or large changes, split them up into tasks and test & commit to git after each one.
- Use commit message format `type: message` where type is fix/feat/docs/...
- Do not perform any verification beyond running the verify script. Do not additionally run `cargo build`, `cargo run`, `cargo test`, or the app itself to check your work — the verify script is the only verification step.
- Do not use the browser tool.

If the VM runs out of space, delete build/debug/incremental.
