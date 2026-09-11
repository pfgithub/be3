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

## Selectors

A list of N rows that each ask "am I the selected one?" through a memo wakes all
N of them every time the selection moves. `create_selector(|| ...)` turns that
into two: it keeps one subscriber list per key that has been asked about, and
when its source changes it notifies only the key that lost selection and the key
that gained it.

```rust
let (selected, set_selected) = create_signal(Some(0usize));
let selection = create_selector(move || selected.get());

for index in 0..rows {
    let selection = selection.clone();
    create_effect(move || highlight(index, selection.is_selected(&Some(index))));
}
```

`is_selected(&key)` subscribes the current computation to that key alone, never
to the source, so a computation reading it reruns only when its own answer
flips. `memo(key)` wraps one key in a `Memo<bool>` for props and handles that
want a value rather than a call. Keys need `Clone + Eq + Hash`, and the source's
own value is its key type, so an optional selection is a `Selector<Option<K>>`
queried with `is_selected(&Some(key))`.

Reads are current the moment they happen, including inside a batch that has not
flushed yet, and a key is forgotten once the last computation watching it is
disposed or stops reading it. Like memos, a selector must be created inside a
scope, and its per-key notifications stop when that scope is disposed.

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

Inside a computation, the current scope is that computation's execution scope,
which is thrown away every time it reruns. To open a scope that survives those
reruns but still belongs to the surrounding tree, use `owner_scope()`: it
returns the `ScopeContext` of the scope that created the running computation
(or the current scope when no computation is running). `ScopeContext::child()`
opens a child scope there, returning `None` if that scope is already gone, and
`ScopeContext::is_alive()` answers the same question on its own. This is how a
list keeps per-item scopes across updates of the list itself while still
tearing them down with whatever owns the list.

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
`create_selector`, `Scope`, `batch`, `untrack`, and `on_cleanup` from this crate) binds signals and
memos directly to `Document` nodes. Nothing in a view takes a `&mut Document`
parameter, so tags nest the way JSX or solidjs would nest them: write the tree
with `view!` inside a `#[component]`, and build the document with `build`, which
supplies the document ambiently and returns the finished `Document`.

```rust
use beui::reactive::{
    build, component, create_memo, create_signal, view, ButtonBuilder, ColumnBuilder, RowBuilder,
    TextBuilder,
};

#[component]
fn app() -> beui::NodeId {
    let (count, set_count) = create_signal(0i64);
    let decrement = set_count.clone();
    let count_text = create_memo(move || count.get().to_string());
    view! {
        <column spacing={0.0}>
            <row spacing={8.0}>
                <button on_click={move || decrement.update(|count| *count -= 1)}>
                    <text string={"-".to_string()} />
                </button>
                <text string={count_text} /> // updates itself when `count` changes
                <button on_click={move || set_count.update(|count| *count += 1)}>
                    <text string={"+".to_string()} />
                </button>
            </row>
        </column>
    }
}

let document = build(|| view! { <app /> });
```

`#[component]` turns a function into a `<tag>` usable from `view!`, by
generating a `NameBuilder` that `view!` fills in. A prop typed `Prop<T>` accepts
either a plain `T` or a signal or memo of `T`; the reactive forms create an
effect that keeps that property in sync. `#[component(base)]` is for the base
elements, which do the same but without a shadow node of their own, and are the
only place that touches `Document` directly. Every builder also accepts
`test_id` and `node_ref`.

`@intrinsic`/`@fixed(size)`/`@percent(weight)` prefix a child inside `view!` to
give it an `ItemSize` in a `row`/`column`. `bind(|document| { ... })` is a
lower-level escape hatch for driving a node property from an effect. See
`crates/beui/examples/counter.rs` for a full example and
`crates/beui/src/document/tests/a_reactive_tree_can_nest_builder_calls_without_threading_the_document.rs`
and `.../a_signal_write_from_a_click_handler_updates_its_bound_text_in_the_same_frame.rs`
for the behavior they rely on.

Each `Document` owns a root `reactive::Scope` (`Document::reactive_scope`), and
every `#[component]` owns a scope of its own, registered against the shadow node
that represents it. A component's scope is a child of the scope that built it,
so component scopes form the same tree the nodes do. `Document::remove_node`
disposes the scopes registered against the subtree it removes: the effects that
were created while building those nodes stop, and the `on_cleanup` callbacks
they registered run. Without that, an effect left alive by a removed component
panics with "node was removed" the next time one of its inputs changes.

Effects created outside any component body — directly in `build`'s closure, for
instance — belong to the document's root scope and live as long as the document.
`show`, `for_each`, and `virtual_list` open a scope per child they build,
registered against that child's node, so dropping a row or scrolling one out of
view disposes exactly that row's effects. Any other code that builds a subtree
it will later remove on its own must do the same, with `in_new_scope`; building
it in the enclosing component's scope instead leaves the subtree's effects alive
after `remove_node` and they panic the next time an input changes.

None of these functions hold `&mut Document` across a call boundary: each reads
it back out of a thread-local (`with_document`) installed by whichever ambient
context is active, and releases it before returning. `build` installs the
document (and enters its scope, so a `Prop` bound to a signal can create its
effect) for the duration of the tree-building closure. `Document::show` installs
itself before dispatching interaction events and flushes queued effects
immediately after, before the frame's paint check, so a signal write from a
click handler is visible in the same frame.

`with_document` asserts when no document is installed at all; nested calls are
fine, and reach the same installed document. Event handlers are plain `FnMut`
closures with no document parameter, so they call ambient functions like any
other code. Keep those calls at the leaves: a component should read and write
node properties through props on base elements, and reach for `with_document`
only inside a base element or to look up its own component state. Effects always
run after the batch that queued them has released its borrow, so ambient calls
inside an effect body nest safely too.
