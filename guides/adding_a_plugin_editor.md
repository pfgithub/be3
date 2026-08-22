1. Add the block (if the block type is new)

- crates/block-client/src/blocks/foo.rs — state struct, operation enum, impl Block with a fresh TYPE_ID UUID, #[cfg(test)] mod tests;.
- crates/block-client/src/blocks.rs — add foo::Foo; to the block_types! list (alphabetical), with : Default when the new-block menu can make one from its default.
- crates/block-client/src/blocks/foo/tests.rs + one file per test under tests/ (serialization round trip, operation edge cases).
- A block holds what it is, not what an editor makes of it. The image block keeps the file's bytes and a metadata field saying what decoding them found — or why it failed — which the editor fills in through an operation the first time it draws the image. Nothing but the plugin then needs a decoder, and no other client has to decode the block to know its shape.

2. Create the plugin package

- crates/editors/foo/Cargo.toml — copy checklist's: crate-type = ["cdylib", "rlib"], a [[bin]] name = "foo-host", deps block-client, block-editor-plugin, uuid. The package name is the wasm artifact stem, so a multi-word editor uses underscores (workspace_index, with a workspace_index-host binary). It also has to be a name no crate the workspace depends on already has, so the image editor's package is image_block; the display name in the manifest is what the user sees.
- crates/editors/foo/src/lib.rs — block_editor_plugin::plugin!(app::FooApp, "../manifest.json");
- crates/editors/foo/src/main.rs — fn main() { #[cfg(not(target_arch = "wasm32"))] foo::run(); }.
- crates/editors/foo/src/app.rs — impl block_editor_plugin::App: connect stores client.get_block(block_id), then ui, toolbar_ui, left_sidebar_ui, right_sidebar_ui, preview_ui reading through block.read() and writing through block.operate(...). Local-only view state lives in the struct. Every region is drawn by an egui context of its own, so anything uploaded to a context — a texture above all — is kept per region rather than once for the app. block_editor_plugin installs the Material icon font into every plugin context and re-exports egui_material_icons, so icons come from block_editor_plugin::egui_material_icons::icons rather than a dependency of the package.
- crates/editors/foo/manifest.json — the plugin's identity and everything the host is told about it (see below).
- Root Cargo.toml — add "crates/editors/foo" to members.

What connect's EditorHost offers, for anything the plugin cannot do itself:

- open_block(id, block_type) — opens another block in a tab of its own.
- block_types() — the host's registered block types, for naming and illustrating a block this editor only holds a reference to. Pass it to block_ui::BlockLabel (re-exported as block_editor_plugin::block_ui) so labels match the rest of the app, including how an automatic name is italicized.
- editable() — whether the block this instance was opened on may be edited, so an editor does not write back what it worked out (an image's decoded size) on behalf of a reader. The host cannot disable a plugin's own surface the way it greys out an editor it draws itself, so an editor whose block may only be read keeps every gesture that would change it behind this: the pixel art editor still pans, zooms and reads out pixels, but draws nothing and disables the buttons that would.
- drag() — the block the app is dragging over the region being drawn, in that region's own coordinates, and whether it has been let go. Draw the drop feedback yourself and answer accept_drag(bool), which decides the cursor the host shows.
- pick_file(filter) — the host's own file picker, the only one that works on desktop, Android and the browser alike. block_editor_plugin::FilePicker wraps it the way the app's own picker works: open(&host, filter), is_open(), and poll(&host) until it answers.
- set_creation_ready(bool) — whether a creation dialog has been filled in, which is what lets the user accept it (see below).

App::intrinsic_size reports the size the editor wants wherever the host embeds it (a canvas, a text block). App::aspect_ratio reports the shape of the block, which the host holds a preview to. Leave either unimplemented to take the host's default.

Drawing the block itself: an editor that lists EditorRegion::Preview is asked to draw its block wherever the host paints it rather than opening it — a canvas entity, a slide, a block embedded in text. The host maps that region onto the quad it is painting, straight from the plugin's surface, so preview_ui fills the region it is given and lets the host place, rotate and fade it. The region has no background of its own, so whatever is behind the block shows through.

Making a new block: the plugin makes every block of its type, so an editor the new-block menu offers implements connect_creation and create_block whatever its creation mode. The host opens an instance with a client of its own but no block, and create_block makes the block through that client and answers with its id or with why it could not be made. Create it orphaned and leave it that way: the host adds the reference and sets the parent once it has the id, and then opens the block, so one plugin runtime serves the creation and the editor that follows it.

Artifacts the editor generates: a block type that produces dynamic artifacts — a PNG exported from a drawing — lists ArtifactSettings among its regions, which is what tells the host this plugin answers for the artifacts its blocks make. The editor creates one through its own client with create_dynamic_artifact, putting whatever it needs in the descriptor's opaque payload, and asks the host to open it. The host then keeps a second instance of the plugin open for as long as that artifact is on screen and asks it, through connect_artifact, describe_artifact, artifact_settings_ui, regenerate_artifact and poll_artifact, which block the payload was generated from and what it currently produces, how to edit it inside the host's settings dialog, and how to rebuild the artifact. The host owns the settings it has stored and hands them over whenever they change; the settings region edits a copy of them, which the host only stores once the user applies it, and throws away when the dialog is dismissed. Regeneration writes the artifact block through the instance's own client, so it polls like any other work waiting on a block to load (see below).

An editor whose block cannot exist until the user has chosen something (the file behind an image) sets "creation": "Dialog" and also implements creation_ui, which draws inside the host's shared dialog frame; set_creation_ready(true) tells the host the dialog may be accepted, and create_block is called on acceptance. An "Immediate" editor is asked for its block as soon as the instance is open, and the host shows only a spinner while it starts.

3. Write the manifest

manifest.json is the plugin's single source of truth: the plugin reads its own identity out of it and the host reads everything else. Nothing in block-app is touched to add a plugin.

```json
{
  "id": "be3.foo",
  "name": "Foo",
  "version": "0.1.0",
  "block_type": "00000000-0000-0000-0000-000000000000",
  "display_name": "Foo",
  "icon": "\ue3f4",
  "creation": "Immediate",
  "regions": ["Main", "Toolbar"],
  "entry_points": { "web": "foo.js", "windows": "foo-host.exe", "linux": "foo-host" },
  "surfaces": ["WebExternalImage", "WindowsDxgi", "LinuxDmaBuf"]
}
```

block_type is the uuid of a block type block-client declares — a plugin cannot bring a block type of its own, which is what keeps child operations, history and duplication working. icon is a codepoint of the app's Material font, not a name to look up. Entry points are resolved against the directory the manifest was found in.

creation is Immediate for a block a user can make from the new-block menu with nothing to fill in first, Dialog for one the editor has to ask about first, or None for one only another block ever produces. The optional fields default to the plainest answer: children says which of the host's structural edits the block type accepts, important puts it in the main section of the add-block picker, interaction says whether an embedded instance is live or only previewed until it is focused, capabilities carries rotation, aspect ratio and pan-and-zoom, and resize says how an embedded instance may be resized.

A cursor an editor sets while drawing a region — ui.ctx().set_cursor_icon(...) — is passed to the host, which shows it while the pointer is over that region, so a canvas can offer a crosshair or a grabbing hand without knowing anything about the window it is in.

capabilities.pan_and_zoom means the editor pans and zooms itself: the host gives it the whole tab viewport and every input event landing in it, rather than transforming the region on the editor's behalf, which would only stretch the surface the plugin renders into. Such an editor keeps its own zoom and offset, redraws its content at the resolution it is being shown at, and offers whatever "fit" control it wants (the PDF editor puts one in its toolbar). Input arrives as ordinary egui events, so a wheel turned with the zoom modifier held and a pinch gesture both read back as ctx.input(|input| input.zoom_delta()).

A manifest is untrusted input, so a bad one is skipped and reported in the Plugins debug window rather than crashing the app.

The host discovers plugins in plugins/<plugin id>/ beside its executable and in a per-user plugins directory, from plugins/index.json on the web, and from the same layout mirrored into assets on Android. Registration is not gated by platform: a plugin host exists for wasm, Windows and Linux, and platforms without one (plugin_host/unavailable.rs) still open the block, drawing an error in place of the plugin's surface. Creating one there fails with that error instead, since only the plugin makes its blocks.

4. Extending the protocol

If the editor needs something the host has and the plugin does not, add it to crates/block-plugin-api (a Message or EditorMessage variant, plus validate coverage and a round-trip test), accept it in the plugin's ClientSession state machine, route it in crates/block-app/src/plugin_host (instances.rs, and the web adapter's receive_all for anything the plugin sends back), and surface it on EditorHost or App. Bump PROTOCOL_VERSION and describe the new rule in crates/block-plugin-api/PROTOCOL.md. Anything the host and a plugin both draw — labels, shared painting helpers — belongs in crates/block-ui, which both depend on.

5. Work that does not finish in one frame

A plugin runtime draws when the host sends it something or when the frame it last drew asked to be drawn again, so a repaint requested from a worker thread does not wake it on its own. An editor doing work off the frame — the PDF editor renders its pages on a thread of its own — polls instead: while a job is outstanding it calls ui.ctx().request_repaint_after(...) with an interval matched to the work, and stops asking once the result is in, so an idle editor costs nothing.

A dependency that only exists on some targets belongs behind a [target.'cfg(...)'.dependencies] table with a stand-in module for the rest, the way the PDF editor keeps PDFium out of its wasm build. A native library the plugin loads at runtime is looked for beside the plugin's own executable and, since a plugin lives in plugins/<plugin id>/, in the directories above it where the app's copy sits.

6. Build scripts

Nothing to edit: ./scripts/build scans crates/editors/*/manifest.json for every target and stages plugins/<plugin id>/ off the package name. ./scripts/build --target TRIPLE --release --output DIRECTORY packages the app, the server and every plugin for a cross target.

7. Verification

Run ./scripts/verify, which builds the Linux host and plugin runtime, then ./scripts/build --target web for the wasm side of the same plugin. Windows can't be compiled from this VM, so mistakes in its half only surface in CI.

Each desktop host shares the pixels the plugin drew rather than copying them: Windows hands over a D3D12 texture and a fence, and Linux exports the plugin's Vulkan image as a dma-buf the host imports into a texture of its own (plugin_host/linux.rs and the plugin's linux_surface.rs). Both sides therefore have to be on the same GPU, which the surface descriptor carries the identity of, and the Linux image is linear and unmodified so that either driver lays its rows out the same way. wgpu enables no extension for sharing a fence, so the Linux plugin waits for its own submission to retire before it publishes a frame, and the value it publishes only tells the host which frame it is.
