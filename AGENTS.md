BE3 project

When making changes to serialization formats or network requests, do not consider backwards compatibility with existing clients or data.

After making changes, always run at least:
- cargo fmt
- cargo check
- cargo test -p ...
