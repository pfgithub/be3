BE3 project

When making changes to serialization formats or network requests, do not consider backwards compatibility with existing clients or data.

After making changes, always run at least:
- `cargo fmt`
- `cargo check`
- `cargo nextest run` (Takes around 10 seconds)

Keep test files seperate from code files. Give every test its own seperate file.

Do not run git commands unless asked, unless they are read-only commands. Don't stage changes, commit changes, or push unless asked.