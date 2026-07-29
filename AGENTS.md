BE3 project

When making changes to serialization formats or network requests, do not consider backwards compatibility with existing clients or data.

After making changes, always run: `cargo fmt` and `cargo nextest run`. This will take ~10 seconds excluding compilation time.

Do not perform any manual verification besides for those commands.

Keep test files seperate from code files. Give every test its own seperate file. Do not add tests for GUI features.

Do not run git commands unless asked, unless they are read-only commands. Don't stage changes, commit changes, or push unless asked.

Do not add irrelevant or useless tests. If a change needs manual testing, note what needs testing in your final output.