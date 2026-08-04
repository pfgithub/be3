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

`EditorKind` states only what the editor needs: everything optional has a default. Implement `CreatableEditor` as well when users can create the type themselves; types that only arrive by import or generation, such as images, implement `EditorKind` alone.

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

Set `CAN_ADD_CHILD` or `CAN_DELETE_CHILD` to `true` only when the editor implements the matching `add_child` or `delete_child` method. These methods update the parent block's references and return `Some(true)` when the Files sidebar may complete the parent transfer.

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

Use `registry.register::<...>()` for editors that implement only `EditorKind`.

The registration automatically adds creatable types to block pickers and teaches the app how to open cached blocks of that type.

## 8. Testing and verification

Do not add tests for GUI behavior. Put deterministic data behavior in the block model and test it in `block-client` instead. Note any interactions that still require manual testing in the final handoff.

From the workspace root, run:

```text
cargo fmt
cargo nextest run
```
