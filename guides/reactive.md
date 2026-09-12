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

## Cloning handles into closures

Signals, memos, selectors, and the `Rc` handles components keep are cheap to
clone, and a `move` closure that keeps one has to own its own clone. `clone!`
writes those clones for you: it takes the names to clone, an arrow, and the
expression they are in scope for.

```rust
let text = create_memo(clone!(count -> move || count.get().to_string()));
create_effect(clone!(count state -> move || state.show(count.get())));
```

`clone!(a b -> expr)` expands to `{ let a = a.clone(); let b = b.clone(); expr }`,
so the originals stay usable afterwards and the last closure that needs a handle
can still take it by move.

## Selectors

A list of N rows that each ask "am I the selected one?" through a memo wakes all
N of them every time the selection moves. `create_selector(|| ...)` turns that
into two: it keeps one subscriber list per key that has been asked about, and
when its source changes it notifies only the key that lost selection and the key
that gained it.

```rust
let (selected, set_selected) = create_signal(Some(0usize));
let selection = create_selector(clone!(selected -> move || selected.get()));

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

An effect that finishes its first execution without reading a signal, opening a
scope, or registering a cleanup can never run again, so it is disposed right
there and releases whatever its closure captured. This is what makes it cheap to
bind a property with an effect that may turn out to hold a plain value.

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
`create_selector`, `clone!`, `Scope`, `batch`, `settle`, `untrack`, `on_cleanup`,
`provide_context`, and `use_context` from this crate) binds signals and
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
elements, which do the same but without a shadow node of their own. Every
builder also accepts `test_id` and `node_ref`.

The base elements and the structural primitives that insert and remove nodes for
a living — `show`, `dynamic`, `for_each`, `virtual_list` — are the only code that
touches `Document` directly. Everything above them, unstyled and styled
components and the code that uses them, says what it wants through props on
those tags.

Derive a prop that depends on other signals with `create_memo`, and pass the
memo straight to the prop; a memo only wakes the property when its value
actually changes.

```rust
let fill = create_memo(move || if hovered.get() { HOVER } else { REST });
view! { <fill color={fill} radius={RADIUS}>{child}</fill> }
```

`Prop<T>` reads with `get()`, which subscribes the computation around it, and
with `peek()`, which does not. A component that has to *read* one of its own
props wraps that in a memo it can read and pass on:

```rust
let text = create_memo(move || content.get());
```

When the component also writes that value itself — a toggle whose `checked` prop
seeds state that clicking then changes — seed a signal from `peek()` and keep it
in sync with an effect, so writes from the component's own handlers and writes
from the caller's signal both land in one place. `Prop::map` adjusts the value on
the way in:

```rust
let value = value.map(|value| value.clamp(0.0, 1.0));
let (value_read, set_value) = create_signal(value.peek());
create_effect(clone!(set_value -> move || set_value.set(value.get())));
```

A component's child arity is part of its signature. A prop named `children`
typed `Children` takes however many are written between its tags; typed `Child`
it takes exactly one and arrives as the `NodeId` itself, so wrappers use
`{children}` in their `view!` without unwrapping, and a caller who writes none
gets a panic naming the component. `Option<Child>` is the same for a wrapper
whose child is optional, like `fill` or a `button` that takes `content` instead.

Props that build part of the tree are typed `Render<H>` when the component calls
them once, `RenderFn<H>` when it may call them many times, and `Option<..>` when
they have a default. Their setters take a bare closure, so a component hands
part of its chrome to its caller the way JSX passes children as a function:

```rust
#[component]
fn checkbox(label: Prop<String>, checked: Prop<bool>) -> NodeId {
    view! {
        <toggle
            checked={checked}
            content={move |handle| view! { <checkbox_face handle={handle} label={label} /> }}
        />
    }
}
```

A render prop runs with the component that *wrote* it installed, not the one
that calls it, so `component_detail` inside one labels the outer component.
`component_detail` takes the same thing a `Prop<String>` does — a plain string
or a signal or memo of one — so the label a component shows in the inspector is
written the way its other properties are: `component_detail(checked.map(label))`.
`Func<V, R>` is the same idea for a plain callback that returns a value, like
`for_each`'s `key`. A prop of any of these three types also accepts an already
built `Render`/`RenderFn`/`Func`, which is how a component forwards one it was
given.

State a component keeps for its own handlers belongs in an `Rc` the handlers
capture; `set_component_state` additionally publishes it so that a test holding
the component's `NodeId` can read it back with `Document::component_state`. That
is the only reader: inside the tree, reaching for a `NodeId` to find state again
is a sign the value should have been captured or passed as a prop.

## Controlled state

Anything a component would otherwise poke into a node after the fact is a prop
on the base element instead, so the component owns a signal and the node follows
it. `overlay` takes `open` and `anchor`, `focusable` takes `focused`,
`text_input` takes `value`, and `scroll` takes `offset` and `reveal` (the index
of the child to bring into view). `unstyled::button`, `unstyled::toggle` and
`unstyled::text_input` forward `focused` to the `focusable` underneath them.

These props are edge triggered: the effect behind them runs when the value it
reads changes, so a component that wants to move focus, or close a popup, writes
its signal and lets the effect do the work. Because the document can also change
that state on its own — a click moves focus, a scrim click dismisses an overlay —
pair the prop with the matching callback and write the signal back:
`on_focus_change` for `focused` and `on_dismiss` for `open`. Without the write
back the signal goes stale and the next write of the value it already holds
changes nothing.

A `Selector` is the natural source for `focused` in a list: keep one signal
naming the row that should have focus and give each row `focused={selection
.memo(Row(index))}`, so moving focus wakes only the row that lost it and the one
that gained it.

```rust
<unstyled::button
    focused={focus.memo(Focus::Row(index))}
    on_focus_change={move |has_focus: bool| {
        if !has_focus && state.focus.get_untracked() == Focus::Row(index) {
            state.set_focus.set(Focus::Away);
        }
    }}
/>
```

When the shape of a subtree depends on a value rather than a flag, `dynamic`
rebuilds it: it holds one child, and every time its `value` changes it builds a
replacement from `view` and removes the old one. Use it where a `show` would
need the value itself rather than a boolean, like a menu whose items can be
swapped out.

```rust
<dynamic value={items} view={move |items: Vec<MenuItem>| view! { <menu_list items={items} /> }} />
```

`@intrinsic`/`@fixed(size)`/`@percent(weight)` prefix a child inside `view!` to
give it an `ItemSize` in a `row`/`column`, and `@size(item_size)` takes a whole
`ItemSize` so a child can switch between kinds reactively — a memo that reads
`narrower_than` and returns `ItemSize::Fixed` in a column where it returned
`ItemSize::Percent` in a row, for instance. A percent child takes its share of
what is left over, so it needs a bounded main axis: inside a list that is being
measured intrinsically there is no leftover space to share, and percent children
fall back to their intrinsic length there, the way `height: 50%` of an
auto-height parent does in CSS. A `scroll` measures as nothing, so a percent
scroll inside an intrinsically measured column collapses; give it a fixed length
for that case. See
`crates/beui/examples/counter.rs` for a full example and
`crates/beui/src/document/tests/a_reactive_tree_can_nest_builder_calls_without_threading_the_document.rs`
and `.../a_signal_write_from_a_click_handler_updates_its_bound_text_in_the_same_frame.rs`
for the behavior they rely on.

## Context

`provide_context(value)` stores a value on the current scope, keyed by its type,
and `use_context::<T>()` walks up the owner chain and returns the nearest one, or
`None`. Because component scopes form the same tree the nodes do, a component
reads whatever its ancestors provided, and the effects it creates later — a
`show` branch that builds long after the first frame, for instance — see the same
values, since a computation's execution scope is parented to the scope that
created it. Provide a distinct wrapper type per concern rather than a bare `f32`,
or two providers will collide on the same key.

One ordering rule matters: `view!` builds a tag's children before the tag itself,
so children written inside a provider's angle brackets are built *before* its
body calls `provide_context`. A provider must therefore take the subtree it
covers as a render prop, which the body calls after providing.

## Containers and responsive layout

Layout sizes are available reactively: `node_size(id)` returns a
`ReadSignal<Vec2>` that the document updates from that node's laid-out rect.
`Document::show` runs layout, publishes the sizes that changed, and lets the
effects that woke up rebuild before it lays out again — up to a few passes per
frame — so a size-driven change is visible in the frame that caused it rather
than one frame later. The signal reads `Vec2::ZERO` until the first layout.

`unstyled::container` ties the two together: it measures its own shadow node,
provides that size as `ContainerSize`, and hands the signal to its `content`
render prop. Anything below it can then ask `container_size()`, or
`narrower_than(width)` for a `Memo<bool>` that is true when the nearest container
is narrower than `width` (false when nothing provides a size, and false until the
first layout). Queries answer for the nearest container, so a card that wraps its
contents in a container gets answers about the card, not the window.

`unstyled::stack` consumes that flag: it is a row that turns into a column, with
its children falling back to intrinsic sizing, while the flag is true. It keeps
the same nodes across the switch, so state inside them survives.
`styled::stack` supplies `theme::NARROW_WIDTH` as the default breakpoint, and
`styled::responsive_tabs` swaps a tab bar for a select below one.

Measuring a node whose size depends on its own content — an intrinsically sized
container whose children react to its width — can oscillate. Give containers a
width that comes from their parent.

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
`show`, `dynamic`, `for_each`, and `virtual_list` open a scope per child they
build, registered against that child's node, so dropping a row or scrolling one
out of view disposes exactly that row's effects. Any other code that builds a subtree
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

`settle` runs a closure and then flushes the effects it queued even when an
outer batch is still open. `virtual_list` builds each row inside one, because it
measures the row immediately afterwards and the row's own props are applied by
effects; without it a row would measure as empty and the list would build every
item in the collection on its first frame.

`with_document` asserts when no document is installed at all; nested calls are
fine, and reach the same installed document. Event handlers are plain `FnMut`
closures with no document parameter, so they call ambient functions like any
other code. Keep those calls at the leaves: a component should read and write
node properties through props on base elements, and reach for `with_document`
only inside a base element or to look up its own component state. Effects always
run after the batch that queued them has released its borrow, so ambient calls
inside an effect body nest safely too.
