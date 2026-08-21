1. Add the block (if the block type is new)

- crates/block-client/src/blocks/foo.rs — state struct, operation enum, impl Block with a fresh TYPE_ID UUID, #[cfg(test)] mod tests;.
- crates/block-client/src/blocks/mod.rs — add pub mod foo; (alphabetical).
- crates/block-client/src/blocks/foo/tests.rs + one file per test under tests/ (serialization round trip, operation edge cases).

2. Create the plugin package

- crates/editors/foo/Cargo.toml — copy checklist's: crate-type = ["cdylib", "rlib"], a [[bin]] name = "foo-host", deps block-client, block-editor-plugin, uuid
- crates/editors/foo/src/lib.rs — block_editor_plugin::plugin!(app::FooApp, "be3.foo", "Foo");
- crates/editors/foo/src/main.rs — fn main() { #[cfg(not(target_arch = "wasm32"))] foo::run(); }.
- crates/editors/foo/src/app.rs — impl block_editor_plugin::App: connect stores client.get_block(block_id), then ui, toolbar_ui, left_sidebar_ui, right_sidebar_ui reading through block.read() and writing through block.operate(...). Local-only view state lives in the struct.
- Root Cargo.toml — add "crates/editors/foo" to members.

3. Register it with the host

- crates/block-app/src/editors/plugin/foo.rs — a FooPlugin unit struct implementing PluginPackage: type Block, const ICON, and manifest() built through super::cached_manifest with the plugin id, block type, display name, regions, entry points (/foo.js, foo-host.exe, com.be3.block.plugin.FooService) and surface mechanisms. The manifest must pass validate() — it's asserted at registry construction.
- crates/block-app/src/editors/plugin.rs — add pub(super) mod foo;.
- crates/block-app/src/editors.rs — add registry.register_plugin::<plugin::foo::FooPlugin>();

Nothing else in block-app changes: the generic PluginEditor<P> and the per-package plugin host handle tabs, regions, input, tunnelled client and surface slots.

4. Build scripts

- scripts/build-block-web.sh — add foo to the plugins=(...) array.
- scripts/run-block-app.sh — cargo build -p foo --bin foo-host so the desktop app can launch it.
- Desktop packaging needs no edit: ./scripts/build-plugin.sh --plugin foo --target ... already works off the package name.

6. Docs and verification

- AGENTS.md — one line under Block types and one under Editor types.
- Run ./scripts/verify.sh, then ./scripts/build-block-web.sh, since the plugin host paths only compile for wasm/Windows and verify.sh doesn't cover them. Windows can't be compiled from this VM, so manifest/entry-point mistakes there only surface in CI.

