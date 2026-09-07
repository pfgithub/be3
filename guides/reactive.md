# Reactive core

`crates/reactive` provides a dependency-free, single-threaded reactive graph. It
is intended for retained UI bindings: create a node once, then use an effect to
update its properties when the values it reads change. The crate does not yet
connect to beui's `Document` or change its rendering/event loop.

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

## Future beui integration

A beui adapter can associate scopes with retained view lifetimes, capture node
IDs in effects, apply property changes, and request repaint through `Context`.
Dispose a view's scope when its nodes are removed.

Beui event handlers borrow `&mut Document`. Wrap the entire event dispatch in a
`batch` and release the document borrow before the batch returns, so effects can
apply their updates safely. Alternatively, have effects enqueue document patches
and drain them once event dispatch releases the borrow. Batching only the signal
write while an outer document borrow remains active is insufficient.
