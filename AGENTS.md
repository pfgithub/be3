BE3 project

Do not run git commands unless asked, except read-only commands. Don't stage changes, commit changes, or push unless asked.

Functionality:
- When making changes to serialization formats or network requests, do not consider backwards compatibility with existing clients or data.

Code style:
- Prefer a.rs over a/mod.rs.

Tests:
- Keep test files seperate from code files.
- Give every test its own seperate file named the same as the function inside it. Example: tests for `src/a.rs` should go in `src/a/tests/fn_name_1.rs`
- Do not add tests for GUI features.
- Do not add irrelevant or useless tests. If a change needs manual testing, note what needs testing in your final output.

Verification:
- After making changes, always run: `cargo fmt` and `cargo nextest run`. This will take ~10 seconds excluding compilation time.
- Do not perform any manual verification besides those commands.

