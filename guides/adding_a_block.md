# Adding a block

Blocks are synchronized, serialized data models. A block type belongs in this directory; its UI belongs in `block-app/src/editors`.

## 1. Define the model and operations

Create `my_block.rs` and define:

- The block state. Derive `Clone`, `Serialize`, and `Deserialize`.
- A serializable operation enum describing every supported mutation.
- A history implementation, or use `block::NoHistory` when undo and redo are intentionally unavailable.

```rust
use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct MyBlock {
    value: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum MyBlockOperation {
    SetValue { value: String },
}

impl MyBlock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

impl Block for MyBlock {
    type Operation = MyBlockOperation;
    type History = block::NoHistory;

    // Generate a new, permanent UUID. Never reuse another block type's ID.
    const TYPE_ID: Uuid = Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0001);

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        match operation {
            MyBlockOperation::SetValue { value } => block.value.clone_from(value),
        }
    }
}
```

Keep fields private when callers only need read access, and expose focused getters. `apply_operation` must be deterministic and must safely ignore operations that no longer apply, such as removing an already-removed item.

Blocks have a generic `name` property (a client-interpreted `{manual, value}` pair, opaque to the server) rather than a single hardcoded name. `implicit_name` defaults to `None`, which leaves the property unset until the user renames the block; the UI then falls back to the block type's registered display name. Override it only when the block type can derive something better from its own content:

```rust
fn implicit_name(&self) -> Option<String> {
    (!self.value.trim().is_empty()).then(|| self.value.clone())
}
```

Once a block has been manually renamed, automatic re-derivation stops touching its name - that precedence is handled for you.

## 2. Track block references

If the state contains UUIDs of other blocks, implement `references`. This drives server-side reference validation, backreferences, dependency watches, and parent behavior.

```rust
fn references(&self) -> Vec<Uuid> {
    self.items.iter().map(|item| item.block_id).collect()
}
```

Return each referenced block once unless reference multiplicity has meaning to the server. Preserve a deterministic order. Do not include ordinary UUIDs that are not block references.

When an editor creates a referenced child, it must update the parent block and then call `set_parent` on the child.

If the block type has a natural notion of a "child" reference that the Files sidebar can add, remove, or swap by drag-and-drop, override `add_child`, `delete_child`, and `replace_child` from the `Block` trait. Each returns the operations needed (an empty `Vec` if the child is already in the requested state), or `None` if the block type doesn't support the operation:

```rust
fn add_child(&self, block_id: Uuid) -> Option<Vec<Self::Operation>> {
    if self.items.iter().any(|item| item.block_id == block_id) {
        return Some(Vec::new());
    }
    Some(vec![MyBlockOperation::AddItem { block_id }])
}
```

`BlockHandleAccess` reads the block, calls the override, and applies the resulting operations, so the corresponding `BlockEditor` method never needs to be implemented - its default already forwards to `self.block()`.

## 3. Choose synchronization and history behavior

Blocks are non-CRDT by default. Set `const CRDT: bool = true` only when operations can be transformed or safely merged under concurrent editing. Implement `transform_operation` when local operations need rebasing over remote operations.

For undo and redo, implement `BlockHistory<MyBlock>` and set it as `type History`. A history action should record the smallest reversible change, report an approximate byte size, and emit inverse or forward operations from `operations`. Grouped operations must produce one coherent history action. See `infinite_canvas.rs` and `presentation.rs` for complete examples.

## 4. Export the module

Add the block type to the list in `blocks.rs`, which declares the module and
puts the type in the erased table the app opens blocks through:

```rust
my_block::MyBlock;
```

The block is now available as `block_client::blocks::my_block::MyBlock`.

## 5. Add model tests

Production code ends with a plain test module declaration:

```rust
#[cfg(test)]
mod tests;
```

Place shared imports and child declarations in `my_block/tests.rs`, and give every test its own file named after its function:

```text
my_block.rs
my_block/
  tests.rs
  tests/
    my_block_applies_operations.rs
    my_block_serialization_round_trips.rs
```

Test meaningful behavior such as operation application, invalid or repeated operations, references, serialization, and history. Do not put individual tests in the production file and do not use `#[path]`.

## 6. Verify

From the workspace root, run:

```text
cargo fmt
cargo nextest run
```

If the block needs a UI, continue with the [editor guide](../../../block-app/src/editors/guide.md).
