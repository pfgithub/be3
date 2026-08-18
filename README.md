# BE3

## About

? Why should every app need to reinvent live collaborative editing, permissions, accounts, undo/redo, rich text editing, ...etc

! BE3 aims to solve this

## Development

See .github/workflows/ci.yml for detailed instructions

Basic setup:

install rustup, `rustup default stable`, install cargo-nextest

## Desktop plugin demo

Build the plugin for the same target as `block-app`, optionally staging the
application executable with it:

```sh
./scripts/build-counter.sh --target x86_64-unknown-linux-gnu --profile release --app-executable target/x86_64-unknown-linux-gnu/release/block-app
./scripts/build-counter.sh --target x86_64-pc-windows-msvc --profile release --app-executable target/x86_64-pc-windows-msvc/release/block-app.exe
./scripts/build-counter.sh --target aarch64-apple-darwin --profile release --app-executable target/aarch64-apple-darwin/release/block-app --sign-identity 'Developer ID Application: Example'
```

The default output is `target/counter/<target>/<profile>`. Linux stages
`bin` and `libexec/be3`, Windows uses a private `counter` directory beside
the app, and macOS creates `Block.app/Contents/MacOS`. Use `--output DIRECTORY`
to select another package root and repeat `--runtime-dependency PATH` for each
non-system shared library or DLL. Windows launches without inherited `PATH`
lookup. macOS signs the plugin and then the complete bundle when both
`--sign-identity` and `--app-executable` are supplied.

Script failures distinguish a missing Rust target artifact, nonexistent runtime
dependency, unsupported desktop target, and unavailable macOS signing tool.
After launch, Plugin Demo separately reports the resolved-path launch failure,
handshake or protocol error, unsupported graphics backend, process exit, and
shutdown timeout. Web plugins are built by `./scripts/build-block-web.sh`.
