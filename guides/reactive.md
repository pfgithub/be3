# Reactive core

`crates/reactive` provides a dependency-free, single-threaded reactive graph. It
is intended for retained UI bindings: create a node once, then use an effect to
update its properties when the values it reads change. `beui::reactive` (in
`crates/beui/src/reactive.rs`) is the adapter that connects it to beui's
`Document`; see "beui integration" below.

## Example

```rust
use reactive::{create_effect, create_memo, create_signal, Scope};

let scope = Scope::new();
let (count, set_count) = create_signal(0);

scope.run(|| {
    let text = create_memo(move || format!("Count: {}", count.get()));
    create_effect(move || println!("{}", text.get()));
});

set_count.set(1);
set_count.update(|count| *count += 1);
```

Keep the scope alive for as long as the view exists. Dropping it or calling
`scope.dispose()` stops its computations and runs their cleanup callbacks.
Run `cargo run -p reactive --example retained` for a complete example that
updates retained state and batches changes around a mutable borrow.

## Signals and memos

`create_signal(value)` returns `(ReadSignal<T>, WriteSignal<T>)`. Both handles
are cheaply cloneable, including for values that do not implement `Clone`.
Signals use reference-counted ownership and remain usable as long as a handle
exists, independently of scopes. Handles and scopes cannot cross threads.

- `get()` clones a value and subscribes the current computation.
- `with(|value| ...)` borrows it and subscribes without requiring `Clone`.
- `get_untracked()` and `with_untracked(...)` read without subscribing.
- `set(value)` requires `PartialEq` and notifies only when the value changes.
- `update(|value| ...)` mutates in place, returns the closure's result, and always
  notifies. It works without `PartialEq` or `Clone`.
- `set_unconditionally(value)` replaces and always notifies.

`create_memo(|| ...)` creates a read-only cached computation with `PartialEq`
output. It computes once immediately, then refreshes on demand. Memos expose the
same reading methods as signals. A changed dependency invalidates downstream
computations, but equal memo outputs suppress downstream execution. Reading a
memo inside a batch returns its current value. Dependency chains and diamonds
refresh before effects observe them, preventing intermediate derived values.

Dependencies are discovered on each execution. Conditional branches unsubscribe
from inputs they no longer read. `untrack(|| ...)` disables subscription for its
closure while preserving the current cleanup scope. Memo computations must be
pure: writing a signal inside a memo panics, including inside `untrack`.

`with` holds a shared borrow for the closure; `update` holds a mutable borrow.
Do not access the same signal incompatibly from those closures.

## Effects, batches, and ownership

Create memos, effects, and cleanup callbacks inside `Scope::run` or another
computation. `create_effect(|| ...)` schedules an initial execution, then runs
again when its tracked inputs change. Effects run synchronously before a write
returns, except inside `batch` or `Scope::run`, which flush when their outermost
batch finishes. Multiple writes coalesce into one pending execution. Effects
created inside effects run after their parent finishes.

`create_effect` returns an `Effect` handle for explicit `dispose()` and
`is_disposed()` checks. Dropping this handle does not stop the effect; its scope
owns it. Effects may write signals, and resulting work is queued rather than
recursively executed. An effect exceeding 10,000 executions in one flush panics and is disposed
to stop runaway feedback loops.

A `Scope::new()` created inside an active scope becomes its child. Parent disposal
also disposes children, even when a child handle remains alive. A scope cannot
be entered after disposal; a memo cannot be read after its owning scope is
disposed. Disposal is idempotent.

`on_cleanup(|| ...)` registers a callback on the current scope. Every computation
has a fresh execution scope: before it reruns, its old nested computations and
cleanups are disposed. Cleanups run in reverse registration order without
tracking reads. All cleanup callbacks are attempted even if one panics; the first
panic is propagated unless disposal is already unwinding.

Tracking, ownership, and scheduler state are restored when user code panics.
A batch that panics does not flush effects while unwinding; pending work runs at
the next successful outer batch or write. A panicking `update` invalidates its
value because it may already have mutated it. There is no transaction rollback.

## beui integration

`beui::reactive` (re-exporting `create_signal`, `create_effect`, `create_memo`,
`Scope`, `batch`, `untrack`, and `on_cleanup` from this crate) binds signals and
memos directly to `Document` nodes. Its builder functions (`text`, `row`,
`column`, `button`, `bind`, ...) do not take a `&mut Document` parameter, so
calls nest the way JSX or solidjs would nest them: build the tree inside
`build`, which supplies the document ambiently and returns the finished
`Document`.

```rust
use beui::reactive::{build, button, column, create_signal, intrinsic, on_click, row, text};

fn app() -> beui::NodeId {
    let (count, set_count) = create_signal(0i64);
    let set_count_decrement = set_count.clone();
    column(0.0, [intrinsic(row(8.0, [
        intrinsic(button(text("-"), on_click(move || set_count_decrement.update(|count| *count -= 1)))),
        intrinsic(text(count)), // updates itself when `count` changes
        intrinsic(button(text("+"), on_click(move || set_count.update(|count| *count += 1)))),
    ]))])
}

let document = build(app);
```

`text(value)` accepts a plain `&str`/`String` or a `ReadSignal<T>`/`Memo<T>`
(`T: ToString`); the reactive forms create an effect that keeps the node's
content in sync. `bind(|document| { ... })` is the general form for driving
other node properties (fill color, visibility, and so on) from an effect.
`intrinsic`/`fixed`/`percent` pair a node with an `ItemSize` for `row`/
`column`. See `crates/beui/examples/counter.rs` for a full example and
`crates/beui/src/document/tests/a_reactive_tree_can_nest_builder_calls_without_threading_the_document.rs`
and `.../a_signal_write_from_a_click_handler_updates_its_bound_text_in_the_same_frame.rs`
for the behavior they rely on.

Each `Document` owns a root `reactive::Scope` (`Document::reactive_scope`);
bindings created through `beui::reactive` are owned by it and live for as long
as the document does. None of these functions hold `&mut Document` across a
call boundary: each reads it back out of a thread-local (`with_document`)
installed by whichever ambient context is active, and releases it before
returning. `build` installs the document (and enters its scope, so `text`'s
signal-bound form can create effects) for the duration of the tree-building
closure. `Document::show` installs itself before dispatching interaction
events and flushes queued effects immediately after, before the frame's paint
check, so a signal write from a click handler is visible in the same frame.

The ambient functions are only safe to call while no other code on the stack
already holds a real `&mut Document` to the same document — which is true
inside `build`'s closure and inside any effect body (effects always run after
the batch that queued them has released its borrow), but is not true inside a
click handler, which is handed a real `&mut Document` directly. Handlers that
need to touch the document should use that parameter (or the explicit,
document-taking `unstyled`/`styled`/`Document` APIs), not `with_document` or
the ambient builders.
