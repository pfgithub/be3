# Adding an editor

Editors provide the `egui` UI for block types defined in `block-client`. Each registered editor can be opened as a tab and embedded inside other editors such as presentations and canvases.

## 1. Create the editor module

Create `my_block.rs`. The editor normally owns a typed `BlockHandle` and any local, unsynchronized UI state.

```rust
use block::BlockParent;
use block_client::{
    blocks::my_block::{MyBlock, MyBlockOperation},
    BlockClient, BlockHandle, BlockRelationships,
};
use eframe::egui;
use egui_material_icons::{icons::ICON_DESCRIPTION, MaterialIcon};
use uuid::Uuid;

use super::{
    BlockEditor, CreatableEditor, DirectEditorCapabilities, DirectEditorViewport, EditorAccess,
    EditorAction, EditorKind,
};

impl EditorKind for MyBlockEditor {
    type Block = MyBlock;

    const DISPLAY_NAME: &'static str = "My block";
    const ICON: MaterialIcon = ICON_DESCRIPTION;

    fn open(_client: &BlockClient, block: BlockHandle<MyBlock>) -> Self {
        Self::new(block)
    }
}

impl CreatableEditor for MyBlockEditor {
    fn create(client: &BlockClient) -> Self {
        Self::new(client.create_block(MyBlock::new()))
    }
}

pub(super) struct MyBlockEditor {
    block: BlockHandle<MyBlock>,
}

impl MyBlockEditor {
    fn new(block: BlockHandle<MyBlock>) -> Self {
        Self { block }
    }
}
```

`EditorKind` states only what the editor needs: everything optional has a default.

Pair it with `CreatableEditor` when the block can be created on the spot. When creating it needs something from the user first — a file, a size, a target — implement `ConfigurableEditor` instead and put that in a `CreationOptions` type. The picker then shows the options in a dialog and creates the block only when it is accepted:

```rust
impl ConfigurableEditor for MyBlockEditor {
    type Options = MyBlockOptions;

    fn create(client: &BlockClient, options: MyBlockOptions) -> Result<Self, String> {
        let source = options.source.ok_or("Choose a source first")?;
        Ok(Self::new(client.create_block(MyBlock::from_source(source))))
    }
}

#[derive(Default)]
pub(super) struct MyBlockOptions {
    source: Option<Source>,
}

impl CreationOptions for MyBlockOptions {
    fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        // Draw the options only: the dialog frame, its Create and Cancel
        // buttons, and its errors are shared.
        self.source.is_some()
    }
}
```

Returning `false` from `ui` keeps Create disabled, so `create` runs only against options the editor already called complete.

## 2. Implement the common block behavior

Every editor supplies identity, naming, relationships, and parent updates:

```rust
impl BlockEditor for MyBlockEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    // Continue with the direct-editor methods below.
}
```

The shared `BlockEditor` methods, including history when the block model enables it, are
provided by this accessor.

## 3. Implement the direct editor

Tabs and embedded editors share the direct-editor interface. Declare the editor's layout behavior, then render its main content:

```rust
fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
    DirectEditorCapabilities {
        allow_rotation: false,
        preserve_aspect_ratio: false,
        supports_pan_and_zoom: false,
    }
}

fn direct_editor_intrinsic_size(
    &mut self,
    _editors: &mut EditorAccess<'_>,
) -> Option<egui::Vec2> {
    Some(egui::vec2(720.0, 480.0))
}

fn direct_editor_ui(
    &mut self,
    ui: &mut egui::Ui,
    _editors: &mut EditorAccess<'_>,
    _scale: f32,
    _viewport: &mut DirectEditorViewport,
) -> Option<EditorAction> {
    let Some(block) = self.block.read() else {
        ui.spinner();
        return None;
    };
    let mut value = block.value().to_owned();
    drop(block);
    if ui.text_edit_singleline(&mut value).changed() {
        self.block.operate(MyBlockOperation::SetValue { value });
    }
    None
}
```

Use `direct_editor_top_bar`, `direct_editor_left_sidebar`, and `direct_editor_right_sidebar` for controls that belong outside the main content. The corresponding `direct_editor_has_*_sidebar` method must return `true` before a sidebar is shown. Return `EditorAction` for app-level navigation or parenting requests.

`crates/block-ui/src/frame.rs` lays those bands out, not the editor: it decides where the toolbar row, the sidebars and the content go inside the frame the tab hands over, floats the sidebars when the frame is narrow, greys everything when the block is read-only, and draws the trail back out of a frame a sub-editor has taken over. An editor that lets the user select a block inside it — the infinite canvas, the text editor — reports that selection through `direct_editor_frame_child` and draws it with `frame_child_ui` instead of `embedded_editor_ui`. The selected editor is then given the whole frame and every chrome band, and the editor that placed it is told its own chrome is only reserved: the bands keep the geometry they had, nothing is drawn in them, and its content, zoom and pan do not move. `clear_direct_editor_frame_child` is how it lets go again, which the framework calls when the user presses Escape or clicks the exit.

Do not retain a `BlockReadGuard` while operating on the same block or calling into another editor. Copy the required data and drop the guard first.

## 4. Support previews and nested blocks

Implement `render` when the block can provide a passive preview. Presentations, canvases, text embeds, and thumbnails use it. Draw only inside the supplied corners and apply `context.opacity`.

When the block references other blocks:

1. Watch `BlockReferenceList::References(block.id())` in the constructor.
2. Match model UUIDs to the resulting `BlockReference` metadata.
3. Call `editors.ensure(id, block_type)` before rendering or delegating.
4. Use `editors.render` for passive previews or the `direct_editor_*` methods for an embedded live editor.

`EditorAccess` prevents an active editor from recursively borrowing itself. Still exclude direct self-links in pickers and provide a useful fallback when a nested preview cannot render.

## 5. Support child blocks when applicable

Implement `add_child`, `delete_child`, and `replace_child` on the block type itself (see the [block guide](../../../block-client/src/blocks/guide.md)), not on the editor. `BlockEditor`'s versions are plain passthroughs to `self.block()`; set `CAN_ADD_CHILD` or `CAN_DELETE_CHILD` to `true` only when the block type overrides the matching method. They return `Some(true)` when the Files sidebar may complete the parent transfer.

Use a `BlockPicker` for child creation and existing-block links. Show its menu where appropriate:

```rust
self.picker
    .show_menu_excluding(ui, editors.registry(), [self.block.id()]);
```

Then handle it from the editor UI and add the returned ID to the parent model:

```rust
if let Some(result) = self.picker.handle(
    ui.ctx(),
    editors,
    BlockParent::Uuid(self.block.id()),
) {
    self.insert_child(result.id);
}
```

`BlockPicker::handle` creates and registers new editors, runs image import, shows errors, and displays the existing-block picker. It sets the requested parent only for newly created blocks, so linked blocks keep their current parent.

## 6. Generate dynamic artifacts when applicable

A dynamic artifact is a block generated from another block, such as the code exported from a GUI design. Blocks that generate one describe it with a `DynamicArtifactSupport`:

```rust
pub(in crate::editors) const SUPPORT: DynamicArtifactSupport = DynamicArtifactSupport {
    source: |data| MyArtifact::decode(data).map(|artifact| artifact.source),
    summary,
    settings_ui,
    regenerate,
};
```

and return it from the otherwise defaulted `EditorKind` method:

```rust
fn dynamic_artifact() -> Option<DynamicArtifactSupport> {
    Some(dynamic_artifact::SUPPORT)
}
```

The descriptor payload is opaque to the app, so it carries both the source block ID and whatever settings the generator needs. `summary` describes what those settings produce, and `settings_ui` edits the payload in place. The artifact bar shows all of this, saves edited settings back onto the block, and reruns `regenerate`.

## 7. Register the editor

In `editors.rs`:

```rust
mod my_block;
```

Then register it in `EditorRegistry::new`:

```rust
registry.register_creatable::<my_block::MyBlockEditor>();
```

Use `registry.register_configurable::<...>()` for editors that implement `ConfigurableEditor`.

The registration automatically adds creatable types to block pickers and teaches the app how to open cached blocks of that type.

`EditorRegistry::new` has no platform gates: every editor is registered on every platform, and every block type can be created everywhere.

## 8. Editors that need a platform-specific library

Some editors are built on something only one platform has — pdfium for the PDF editor, a native webview for the browser tab. Keep the editor itself platform independent and put only the part that needs the library behind a `cfg`, in a submodule of the editor:

```rust
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod pdfium;
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
mod unsupported;
```

Both submodules expose the same interface, and the `unsupported` one fails with a message saying the platform cannot do it. The editor then works everywhere — it opens, it takes input, its block can be created and edited — and shows that message where the content it cannot produce would go. The platform-specific dependency stays behind the matching `cfg` in `Cargo.toml`.

Dead-code warnings on the platforms without the real implementation are silenced where the item is defined:

```rust
#[cfg_attr(any(target_os = "android", target_arch = "wasm32"), allow(dead_code))]
```

## 9. Testing and verification

Do not add tests for GUI behavior. Put deterministic data behavior in the block model and test it in `block-client` instead. Note any interactions that still require manual testing in the final handoff.

From the workspace root, run:

```text
cargo fmt
cargo nextest run
```
