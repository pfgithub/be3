1. Add the block (if the block type is new)

- crates/block-client/src/blocks/foo.rs — state struct, operation enum, impl Block with a fresh TYPE_ID UUID, #[cfg(test)] mod tests;.
- crates/block-client/src/blocks.rs — add foo::Foo; to the block_types! list (alphabetical), with : Default when the new-block menu can make one from its default.
- crates/block-client/src/blocks/foo/tests.rs + one file per test under tests/ (serialization round trip, operation edge cases).
- A block holds what it is, not what an editor makes of it. The image block keeps the file's bytes and a metadata field saying what decoding them found — or why it failed — which the editor fills in through an operation the first time it draws the image. Nothing but the plugin then needs a decoder, and no other client has to decode the block to know its shape.

2. Create the plugin package

- crates/editors/foo/Cargo.toml — copy checklist's: crate-type = ["cdylib", "rlib"], a [[bin]] name = "foo-host", deps block-client, block-editor-plugin, uuid. The package name is the wasm artifact stem, so a multi-word editor uses underscores (workspace_index, with a workspace_index-host binary). It also has to be a name no crate the workspace depends on already has, so the image editor's package is image_block; the display name in the manifest is what the user sees.
- crates/editors/foo/src/lib.rs — block_editor_plugin::plugin!(app::FooApp, "be3.foo", "Foo");
- crates/editors/foo/src/main.rs — fn main() { #[cfg(not(target_arch = "wasm32"))] foo::run(); }.
- crates/editors/foo/src/app.rs — impl block_editor_plugin::App: connect stores client.get_block(block_id), then ui, toolbar_ui, left_sidebar_ui, right_sidebar_ui, preview_ui reading through block.read() and writing through block.operate(...). Local-only view state lives in the struct. Every region is drawn by an egui context of its own, so anything uploaded to a context — a texture above all — is kept per region rather than once for the app. block_editor_plugin installs the Material icon font into every plugin context and re-exports egui_material_icons, so icons come from block_editor_plugin::egui_material_icons::icons rather than a dependency of the package.
- Root Cargo.toml — add "crates/editors/foo" to members.

What connect's EditorHost offers, for anything the plugin cannot do itself:

- open_block(id, block_type) — opens another block in a tab of its own.
- block_types() — the host's registered block types, for naming and illustrating a block this editor only holds a reference to. Pass it to block_ui::BlockLabel (re-exported as block_editor_plugin::block_ui) so labels match the rest of the app, including how an automatic name is italicized.
- editable() — whether the block this instance was opened on may be edited, so an editor does not write back what it worked out (an image's decoded size) on behalf of a reader.
- drag() — the block the app is dragging over the region being drawn, in that region's own coordinates, and whether it has been let go. Draw the drop feedback yourself and answer accept_drag(bool), which decides the cursor the host shows.
- pick_file(filter) — the host's own file picker, the only one that works on desktop, Android and the browser alike. block_editor_plugin::FilePicker wraps it the way the app's own picker works: open(&host, filter), is_open(), and poll(&host) until it answers.
- set_creation_ready(bool) — whether a creation dialog has been filled in, which is what lets the user accept it (see below).

App::intrinsic_size reports the size the editor wants wherever the host embeds it (a canvas, a text block). App::aspect_ratio reports the shape of the block, which the host holds a preview to. Leave either unimplemented to take the host's default.

Drawing the block itself: an editor that lists EditorRegion::Preview is asked to draw its block wherever the host paints it rather than opening it — a canvas entity, a slide, a block embedded in text. The host maps that region onto the quad it is painting, straight from the plugin's surface, so preview_ui fills the region it is given and lets the host place, rotate and fade it. The region has no background of its own, so whatever is behind the block shows through.

Filling in a new block: an editor whose block cannot exist until the user has chosen something (the file behind an image) sets CreationMode::Dialog and implements connect_creation, creation_ui and create_block. It has a client of its own but no block, and draws inside the host's shared dialog frame. set_creation_ready(true) tells the host the dialog may be accepted; on acceptance the host calls create_block, which makes the block through the instance's own client and answers with its id or with why it could not be made. The host then opens the block it was given, so one plugin runtime serves the dialog and the editor that follows it.

3. Register it with the host

- crates/block-app/src/editors/plugin/foo.rs — a FooPlugin unit struct implementing PluginPackage: type Block, const ICON, new_block (the block the new-block menu makes, or None when a dialog fills it in), and manifest() built through super::cached_manifest with the plugin id, block type, display name, regions, entry points (/foo.js, foo-host.exe) and surface mechanisms. Use CreationMode::Immediate for a block a user can make from the new-block menu, CreationMode::Dialog for one the editor has to ask about first, or CreationMode::None for one only another block ever produces. children says which of the host's structural edits the block type accepts, important puts it in the main section of the add-block picker, interaction says whether an embedded instance is live or only previewed until it is focused, capabilities carries rotation, aspect ratio and pan-and-zoom, and resize says how an embedded instance may be resized. The manifest must pass validate() — it's asserted at registry construction.
- Anything else in block-app that made this block type — importing an image onto a canvas, say — creates a PluginEditor<FooPlugin> and hands it to EditorAccess::insert. Keep those helpers next to the package's registration.
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
