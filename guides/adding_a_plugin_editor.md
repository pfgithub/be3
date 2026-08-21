1. Add the block (if the block type is new)

- crates/block-client/src/blocks/foo.rs — state struct, operation enum, impl Block with a fresh TYPE_ID UUID, #[cfg(test)] mod tests;.
- crates/block-client/src/blocks/mod.rs — add pub mod foo; (alphabetical).
- crates/block-client/src/blocks/foo/tests.rs + one file per test under tests/ (serialization round trip, operation edge cases).

2. Create the plugin package

- crates/editors/foo/Cargo.toml — copy checklist's: crate-type = ["cdylib", "rlib"], a [[bin]] name = "foo-host", deps block-client, block-editor-plugin, uuid. The package name is the wasm artifact stem, so a multi-word editor uses underscores (workspace_index, with a workspace_index-host binary).
- crates/editors/foo/src/lib.rs — block_editor_plugin::plugin!(app::FooApp, "be3.foo", "Foo");
- crates/editors/foo/src/main.rs — fn main() { #[cfg(not(target_arch = "wasm32"))] foo::run(); }.
- crates/editors/foo/src/app.rs — impl block_editor_plugin::App: connect stores client.get_block(block_id), then ui, toolbar_ui, left_sidebar_ui, right_sidebar_ui reading through block.read() and writing through block.operate(...). Local-only view state lives in the struct. block_editor_plugin installs the Material icon font into every plugin context and re-exports egui_material_icons, so icons come from block_editor_plugin::egui_material_icons::icons rather than a dependency of the package.
- Root Cargo.toml — add "crates/editors/foo" to members.

What connect's EditorHost offers, for anything the plugin cannot do itself:

- open_block(id, block_type) — opens another block in a tab of its own.
- block_types() — the host's registered block types, for naming and illustrating a block this editor only holds a reference to. Pass it to block_ui::BlockLabel (re-exported as block_editor_plugin::block_ui) so labels match the rest of the app, including how an automatic name is italicized.
- drag() — the block the app is dragging over the region being drawn, in that region's own coordinates, and whether it has been let go. Draw the drop feedback yourself and answer accept_drag(bool), which decides the cursor the host shows.

App::intrinsic_size reports the size the editor wants wherever the host embeds it (a canvas, a text block). Leave it unimplemented to take the host's default.

3. Register it with the host

- crates/block-app/src/editors/plugin/foo.rs — a FooPlugin unit struct implementing PluginPackage: type Block, const ICON, and manifest() built through super::cached_manifest with the plugin id, block type, display name, regions, entry points (/foo.js, foo-host.exe) and surface mechanisms. Use CreationMode::Immediate for a block a user can make from the new-block menu, or CreationMode::None for one only another block ever produces. children says which of the host's structural edits the block type accepts, important puts it in the main section of the add-block picker, and resize says how an embedded instance may be resized. The manifest must pass validate() — it's asserted at registry construction.
- crates/block-app/src/editors/plugin.rs — add pub(super) mod foo;.
- crates/block-app/src/editors.rs — add registry.register_plugin::<plugin::foo::FooPlugin>();

Nothing else in block-app changes: the generic PluginEditor<P> and the per-package plugin host handle tabs, regions, input, tunnelled client and surface slots. The registration is not gated by platform: a plugin host exists for wasm and Windows, and platforms without one (plugin_host/unavailable.rs) still create and open the block, drawing an error in place of the plugin's surface.

4. Extending the protocol

If the editor needs something the host has and the plugin does not, add it to crates/block-plugin-api (a Message or EditorMessage variant, plus validate coverage and a round-trip test), accept it in the plugin's ClientSession state machine, route it in crates/block-app/src/plugin_host (instances.rs, and the web adapter's receive_all for anything the plugin sends back), and surface it on EditorHost or App. Bump PROTOCOL_VERSION and describe the new rule in crates/block-plugin-api/PROTOCOL.md. Anything the host and a plugin both draw — labels, shared painting helpers — belongs in crates/block-ui, which both depend on.

5. Build scripts

- scripts/build-block-web.sh — add foo to the plugins=(...) array.
- scripts/run-block-app.sh — cargo build -p foo --bin foo-host so the desktop app can launch it.
- Desktop packaging needs no edit: ./scripts/build-plugin.sh --plugin foo --target ... already works off the package name.

6. Verification

Run ./scripts/verify.sh, then ./scripts/build-block-web.sh, since the real plugin host only compiles for wasm/Windows and verify.sh builds the placeholder host instead. Windows can't be compiled from this VM, so manifest/entry-point mistakes there only surface in CI.
