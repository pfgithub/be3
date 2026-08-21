BE3 project

Guides (read if they are relevant):
- guides/adding_a_block.md
- guides/adding_an_editor.md
- guides/adding_a_plugin_editor.md

Tooling:
- The codebase-memory-mcp knowledge graph indexes this repo under the project key `home-exedev-be3`, not `be3` — pass that key to `search_graph`/`query_graph`/`get_code_snippet`/etc.
- Remember that subagents also read AGENTS.md automatically, so there is no need to reiterate information in here when spawning a subagent.

Functionality:
- When making changes to serialization formats or network requests, do not consider backwards compatibility with existing clients or data.

UI style:
- Never use unicode characters for icons. Always use an icon library instead, or no icon.

Code style:
- Prefer a.rs over a/mod.rs.
- Do not add any comments to the code. Do not add doc-comments either. You may remove existing comments & doc comments.

Tests:
- Keep test files seperate from code files.
- Give every test its own seperate file named the same as the function inside it. Tests for `src/a.rs` go in `src/a/tests/fn_name_1.rs`; test imports and support functions go in `src/a/tests.rs`. Tests for a crate root such as `src/lib.rs` instead go in `src/tests/fn_name_1.rs`, with imports and support functions in `src/tests.rs`. Import test modules with plain `mod tests;` and plain child `mod fn_name_1;` declarations; do not use `#[path]`. Production files only import their `tests.rs` module and do not define every individual test.
- Do not add tests for GUI features.
- Do not add irrelevant or useless tests.
- If a change needs manual testing, note what needs testing in your final output.

Verification:
- These are the allowed commands for verification:
  - `./scripts/verify.sh`: always run this one before committing.
  - `PATH="/home/ubuntu/.local/android-build/gradle-8.11.1/bin:$PATH" ./scripts/build-block-android.sh --android-sdk /home/ubuntu/Android/Sdk`: run this for changes that affect features specific to Android.
  - `./scripts/build-block-web.sh`: run this for changes that affect features specific to web
- Do not perform any further verification. Do not use the browser tool. Do not additionally run `cargo build`, `cargo run`, `cargo test`, or the app itself to check your work. Do not try building for other platforms.
- After running verification, commit and push changes in git.
- When there are multiple or large changes, split them up into tasks and verify, commit, and push after each one.
- Use commit message format `type: message` where type is fix/feat/docs/...
- You may commit and push a change even if it still needs further verification beyond an allowed verify command.

Environment:
- You are inside of an ubuntu VM.
