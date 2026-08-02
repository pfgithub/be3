# Adding an editor

Editors provide the `egui` UI for block types defined in `block-client`. Each registered editor can be opened as a tab and embedded inside other editors such as presentations and canvases.

## 1. Create the editor module

Create `my_block.rs`. The editor normally owns a typed `BlockHandle` and any local, unsynchronized UI state.

```rust
use block::{Block, BlockParent};
use block_client::{
    blocks::my_block::{MyBlock, MyBlockOperation},
    BlockClient, BlockHandle, BlockRelationships,
};
use eframe::egui;
use egui_material_icons::icons::ICON_DESCRIPTION;
use uuid::Uuid;

use super::{
    BlockEditor, DirectEditorCapabilities, DirectEditorViewport, EditorAccess, EditorAction,
    EditorRegistration,
};

pub(super) fn registration() -> EditorRegistration {
    EditorRegistration {
        block_type: MyBlock::TYPE_ID,
        display_name: "My block",
        icon: ICON_DESCRIPTION,
        create: Some(|client| {
            Box::new(MyBlockEditor::new(client.create_block(MyBlock::new())))
        }),
        open: |client, id| {
            Box::new(MyBlockEditor::new(client.get_block::<MyBlock>(id)))
        },
        can_add_child: false,
        can_delete_child: false,
        regenerate_dynamic_artifact: None,
    }
}

struct MyBlockEditor {
    block: BlockHandle<MyBlock>,
}

impl MyBlockEditor {
    fn new(block: BlockHandle<MyBlock>) -> Self {
        Self { block }
    }
}
```

Set `create` to `None` for types users cannot create directly, such as imported images. `open` must always construct an editor for an existing block ID.

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

Use `direct_editor_top_bar`, `direct_editor_left_sidebar`, and `direct_editor_right_sidebar` for controls that belong outside the main content. The corresponding `direct_editor_has_*_sidebar` method must return `true` before a sidebar is shown. Return `EditorAction` for app-level creation, navigation, or parenting requests.

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

Set `can_add_child` or `can_delete_child` in the registration only when the editor implements the matching `add_child` or `delete_child` method. These methods update the parent block's references and return `Some(true)` when the Files sidebar may complete the parent transfer.

For child creation from inside an editor, return:

```rust
EditorAction::CreateBlock {
    block_type,
    parent: Some(self.block.id()),
}
```

Store enough pending local state for `block_created` to insert the newly created block reference, then return `true`. The app will record the child's backreference and parent. For existing linked blocks that should keep their current parent, update only the model reference and do not return `SetParent`.

## 6. Register the editor

In `editors.rs`:

```rust
mod my_block;
```

Then add its registration in `EditorRegistry::new`:

```rust
registry.register(my_block::registration());
```

The registration automatically adds creatable types to block pickers and teaches the app how to open cached blocks of that type.

## 7. Testing and verification

Do not add tests for GUI behavior. Put deterministic data behavior in the block model and test it in `block-client` instead. Note any interactions that still require manual testing in the final handoff.

From the workspace root, run:

```text
cargo fmt
cargo nextest run
```
