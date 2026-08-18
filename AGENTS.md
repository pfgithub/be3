BE3 project

Overview:
- BE3 is a collaborative, block-based application platform: workspaces contain blocks (documents, databases, canvases, logic circuits, etc.) that sync in real time between clients through a server, with permissions, undo/redo, and presence built in once per block type rather than reimplemented per app.
- A "block" is a synchronized, serialized data model with an operation log (see `crates/block-client/src/blocks/guide.md` for how to add one). Blocks can reference other blocks by UUID to form parent/child and dependency relationships. By default a block's operations are serialized one at a time (the client waits for each to be acknowledged before sending the next); a block type may opt into CRDT mode as a performance optimization once its operations can be safely transformed against concurrent remote ones, which lets a client keep sending local operations without waiting on round-trip acknowledgment.
- Presence (cursors, "who's viewing", etc.) is a synchronized key-value map per block, kept separate from the block's persisted operation-log state, identified by well-known UUIDs and cleared once a client stops reporting it.
- Workspace access is controlled by `WorkspaceRole` (Administrator/Editor) combined with per-block `BlockAccess` levels (None/KnowExists/View/Edit).
- Editors are the `egui` UI layer for a block type (see `crates/block-app/src/editors/guide.md`), registered in an `EditorRegistry` and usable both as standalone tabs and embedded inside other editors (e.g. a canvas or presentation rendering a child block's preview).
- Blocks vs. editors: keep a block (`block-client/src/blocks/*.rs`) as minimal as reasonable — just the state and operations needed for synchronization, undo/redo, and (if enabled) CRDT merging. Put the actual logic in the editor (`block-app/src/editors/*.rs`) instead, which reads the block and drives it via `block.operate(...)`. This applies to text too: `blocks/text.rs` is a thin CRDT sequence, while the substantial logic (diffing, highlighting, cursor placement) lives on the editor side, in `text-editor-core`.
- Editors with substantial logic often split it out of their egui code, e.g. `editors/infinite_canvas/core.rs` and `editors/logic_grid/simulation.rs`. `text-editor-core` is the same idea taken further: it was pulled out into its own crate (rather than a submodule under `editors/text/`) so it can be exported and reused outside this project, independent of `block-app`'s UI.
- A "dynamic artifact" is a block generated/derived from another block, such as code exported from the GUI builder.

Main folders:
- `crates/block` — shared protocol and data-model crate with no I/O: `Account`, `Workspace`, permission types, the `Block`/`BlockHistory` traits, and the `ClientMessage`/`ServerMessage` sync protocol. Depended on by both client and server.
- `crates/block-client` — client-side runtime: `BlockClient`/`BlockHandle` (read/operate/undo-redo on a block), the websocket transport (native and web variants), and `blocks/` — one module per block type, listed under "Block types" below. Also owns `properties` (generic block metadata like name) and `presence`.
- `crates/block-app` — the `egui` app (desktop, Android, and web/wasm) that end users run: `editors/` has one module per editor, listed under "Editor types" below, plus `app_state` (local persisted app settings), `block_picker` (create/link-block UI), `files` (workspace tree sidebar), `share` (permission-sharing dialog), `platform` (native vs. web glue), and `debug` (developer-facing inspection tools).
- `crates/block-server` — the sync server: a combined websocket + minimal HTTP server (`http.rs`) backed by SQLite, handling auth, block storage, and broadcasting operations to connected clients.
- `crates/block-e2e` — end-to-end tests that run a real server and client together; no library code of its own beyond test support.
- `crates/text-editor-core` — the text editor's own logic, pulled out of `block-app` into its own crate: text diffing (`core.rs`), `tree-sitter`-based syntax highlighting for Markdown/Rust/Zig (`highlighter/`), and text-cursor presence. It sits on the editor/view side, not the block side — the `text` block itself (`block-client/src/blocks/text.rs`) stays minimal.
- `crates/logicgame` — a standalone logic-circuit simulation engine (`grid` for the component graph, `execution` for running it, `challenges` for the built-in puzzle set), used by the logic_game/logic_grid/compiled_logic block types but with no dependency on `block`/`block-client` itself.
- `scripts/` — verification, platform build scripts, and the `web/` wasm/WASI host glue for the browser build.
- `crates/citygame`, `crates/citygenerator`, `crates/reactive`, `crates/tablet`, `crates/cvl2` — separate, unrelated projects that happen to live in this workspace; ignore them for BE3 work.

Block types (the core, in `crates/block-client/src/blocks/<name>.rs`; each is defined by a state struct, an operation enum, and `apply_operation`):
- `audio` — an uploaded audio clip (source name, media type, raw bytes). No undo/redo history.
- `calendar` — a list of scheduled events (title, start/end time).
- `compiled_logic` — a `logic_grid` circuit compiled into a reusable component (shape, ports) that can be placed inside other circuits.
- `database` — a database's rows, each a map of field ID to value, against a schema defined by `database_schema`.
- `database_schema` — the field definitions (name, type, enum options) shared by a `database` and its `database_view`s.
- `database_view` — a saved view over a `database`: spreadsheet, kanban, or scatter, its sort order, and, for kanban/scatter, which field(s) drive the layout.
- `gui_builder` — a design-only layout of nested widgets (headings, labels, buttons, text fields, containers).
- `hotbar` — the tool/component palette shared by a game's logic grids, registered under the workspace's root `Settings` block so a pinned component is offered in every circuit.
- `image` — an uploaded raster image (dimensions, media type, raw bytes). No undo/redo history.
- `infinite_canvas` — a freeform 2D canvas of positioned, styled entities (shapes, text, embedded block references).
- `logic_game` — the built-in logic-circuit curriculum: levels tied to `logicgame` challenges, the player's `logic_grid` attempts, and quiz answers.
- `logic_grid` — one logic circuit: components and wires placed on a grid, optionally an attempt at a `logic_game` challenge.
- `map` — points of interest and the geographic region shown when the map is previewed or first opened.
- `pixel_art` — a fixed-size indexed-color pixel grid with a bounded palette.
- `pixel_ray_tracer` — a small scene of ray-traced primitives, rendered to a fixed-size pixel buffer.
- `presentation` — an ordered list of slides, each pointing at another block to present.
- `settings` — the workspace's single root block holding every other block type's registered settings, keyed by type ID and activation condition (per-client or fallback), so the root listing stays at one entry regardless of how many settings exist.
- `text` — the collaborative rich-text document (a CRDT sequence via the `eips` crate) backing the text editor and `text-editor-core`.
- `video` — a timeline of clips with frame-accurate rate/length arithmetic.
- `web_browser_tab` — one embedded browser tab's navigation history and its current position in it.
- `workspace_index` — a CRDT set of block IDs used to build workspace-wide listings such as recently deleted.

Editor types (the view, in `crates/block-app/src/editors/<name>.rs`; each reads its block via a `BlockHandle` and sends `operate()` calls rather than owning block logic itself):
- `audio` — playback UI for an `Audio` block.
- `browser_tab` — the embedded native webview for a `WebBrowserTab` block; unavailable on Android/web, where `unsupported` is shown instead.
- `calendar` — day/week/month view over a `Calendar` block's events.
- `compiled_logic` — read-only inspector for a compiled logic component's metadata.
- `database_schema` — add, remove, and rename fields and enum options on a `DatabaseSchema`.
- `database_view` — the `spreadsheet`, `kanban`, and `scatter` layouts over a `Database` through a `DatabaseView`.
- `gui_builder` — the drag-and-drop widget surface (`surface.rs`) and property inspector (`inspector.rs`) for a `GuiBuilder` layout; `dynamic_artifact/` exports it as generated Rust UI code.
- `hotbar` — editor for the shared tool palette: add, remove, and reorder slots and folders.
- `image` — viewer and import UI for an `Image` block.
- `infinite_canvas` — the freeform canvas editor; see the internal `core.rs`/view split noted above for how it divides into `core.rs`, `interaction.rs`, `painting.rs`, `inspector.rs`, and `geometry.rs`.
- `logic_game` — level-select and quiz UI for the built-in logic curriculum (`binary_addition.rs`).
- `logic_grid` — the circuit editor: `canvas.rs` (input), `render.rs`/`renderer/` (GPU-accelerated drawing via a `wgsl` shader), `simulation.rs` (running the circuit), `challenge.rs` (testing against a `logic_game` level), `hotbar.rs` (palette), `geometry.rs` (shared math).
- `map` — the map viewer/editor: `raster.rs`/`mvt.rs` (tile rendering backends), `tiles.rs` (fetching/caching), `points.rs` (points of interest), `sidebar.rs`, `geo.rs` (projection math).
- `pixel_art` — the pixel-grid painting editor; `dynamic_artifact/` exports it as a PNG `Image` block.
- `pixel_ray_tracer` — scene editor for the ray-traced pixel block; `raytracer.rs` renders the live preview.
- `presentation` — slide list and full-screen presenter view.
- `settings` — editor(s) for entries registered into the root `Settings` block.
- `text` — the `text-editor-core`-backed rich text editor: `font.rs` (font handling), `timings.rs` (typing/highlight performance).
- `video` — `timeline.rs`, `player.rs`, and `effects.rs` for a `Video` block.
- `workspace_index` — folder-style listing UI over a `WorkspaceIndex`.
- `clipboard` — not a block editor: a shared helper for pasting images from the OS clipboard into editors such as `infinite_canvas`.
- `unsupported` — fallback editor shown for a block type this build has no editor registered for.

Tooling:
- The codebase-memory-mcp knowledge graph indexes this repo under the project key `home-exedev-be3`, not `be3` — pass that key to `search_graph`/`query_graph`/`get_code_snippet`/etc.
- Remember that subagents also read AGENTS.md automatically, so there is no need to reiterate information in here when spawning a subagent.

Functionality:
- When making changes to serialization formats or network requests, do not consider backwards compatibility with existing clients or data.

UI style:
- Never use unicode characters for icons. Always prefer icon libraries / plain text.

Code style:
- Prefer a.rs over a/mod.rs.
- Do not add comments to the code. This includes documentation comments too, do not add doc comments. You may also remove existing code comments.

Tests:
- Keep test files seperate from code files.
- Give every test its own seperate file named the same as the function inside it. Tests for `src/a.rs` go in `src/a/tests/fn_name_1.rs`; test imports and support functions go in `src/a/tests.rs`. Tests for a crate root such as `src/lib.rs` instead go in `src/tests/fn_name_1.rs`, with imports and support functions in `src/tests.rs`. Import test modules with plain `mod tests;` and plain child `mod fn_name_1;` declarations; do not use `#[path]`. Production files only import their `tests.rs` module and do not define every individual test.
- Do not add tests for GUI features.
- Do not add irrelevant or useless tests. If a change needs manual testing, note what needs testing in your final output.

Verification:
- After making changes, always run `./scripts/verify.sh`. It runs clippy (applying the fixes it can), rustfmt and the tests, and fails if any clippy warning is left. This should take less than 2 minutes including compilation time. Then, commit changes to git and push using `git push`.
- For changes that affect Android, also build the Android APK locally with `PATH="/home/ubuntu/.local/android-build/gradle-8.11.1/bin:$PATH" ./scripts/build-block-android.sh --android-sdk /home/ubuntu/Android/Sdk`.
- For changes that affect the web build, also build it locally with `./scripts/build-block-web.sh`.
- Commit using `git add --all` and `git commit`. Don't check the status. Don't worry about it if the wrong file ends up in a commit unless it is supposed to be gitignored.
- When there are multiple or large changes, split them up into tasks and test & commit to git after each one.
- Use commit message format `type: message` where type is fix/feat/docs/...
- Except for the required Android and web builds above, do not perform any verification beyond running the verify script. Do not additionally run `cargo build`, `cargo run`, `cargo test`, or the app itself to check your work.
- Do not use the browser tool.

Environment:
- You are inside of an ubuntu VM.
