# Beui

Beui is BE3's user interface toolkit. It is a solidjs-like framework: a
`Document` owns a persistent tree of nodes, `view!` builds that tree once, and
reactive signals update the nodes in place afterwards. There is no virtual DOM,
no diffing, and no per-frame rebuild. Building the view is not the render loop:
build it once, keep the `Document`, and call `Document::show` with it every
frame to lay out, dispatch input, expose accessibility, and paint.

The consequence worth internalising is that a component body runs **once**. The
closures and effects it leaves behind are what run again. Everything else in
this guide follows from that.

The quickest introductions are `crates/beui/examples/counter.rs` and the
component catalog in `crates/beui/examples/demo.rs`. The
[reactive guide](reactive.md) is the reference for signals, attribute syntax,
children and render props, controlled state, keyed lists, scopes, and context;
this guide is about using beui itself.

## The rules that matter most

These four cover most review comments on beui code.

### Every function that returns a `NodeId` is a `#[component]`

If a function returns a `NodeId`, annotate it `#[component]` and name it in
`CamelCase`. The attribute is not decoration. It gives the function its own
reactive scope, registered against the node it returns, so that removing that
node disposes exactly the effects the function created. A plain helper that
builds nodes leaves its effects in the caller's scope, where they outlive the
subtree they bind and panic with "node was removed" the next time one of their
inputs changes. `#[component]` is also what makes the function usable as a tag,
gives it `@test_id`, `@node_ref` and `@sizing`, and makes
`component_state`, `component_accessibility` and `component_size` available
inside it.

Functions that return something other than a `NodeId` are ordinary functions.
Deriving a colour from theme tokens and interaction state, mapping a value to a
label, reading state back out of a built node — write those as plain functions,
as `styled/checkbox.rs` does with `box_fill` and `checkbox_checked`.

### A component ends with one `view!` and nothing after it

The last line of a component is a single `view! {}` producing the node it
returns:

```rust
#[component]
fn LabeledValue(label: Prop<String>, value: Prop<String>) -> NodeId {
    let text = create_memo(move || value.get());
    view! {
        <Column spacing=4.0>
            <Text string={label} />
            <Text string={text} />
        </Column>
    }
}
```

Signals, memos, callbacks, and handle destructuring go above it. Nothing goes
below it, and there is no second `view!` earlier in the body: `view!` builds
nodes the moment it runs, so a subtree built into a local and then used
conditionally has already been added to the document whether or not it ends up
in the tree, and a subtree built in one place but parented somewhere else
obscures which component's scope owns it.

When part of the tree depends on something, express it in the view rather than
in Rust control flow around it. `Show` takes a condition and builds its child
lazily the first time it becomes true, `Dynamic` rebuilds a subtree when a value
changes shape, `Keyed` rebuilds only when its key changes, and `ForEach` keeps a
keyed child per item. A component that is genuinely two different trees is two
components with a `Dynamic` or a `Show` choosing between them.

The exception is a component whose root is a base node it creates directly —
the base layer itself, where `List` calls `create_list` and binds setters with
effects. Above the base layer, one `view!` is the shape.

### Call components from `view!`, never their builders

`#[component] fn Button` also generates `ButtonBuilder` and a `Button()`
constructor returning it. That is the machinery `view!` writes into; it is not
an API to call by hand. Write

```rust
view! {
    <Button label="Save" variant=ButtonVariant::Primary on_click={save} />
}
```

and not `Button().label("Save").variant(...).build()`. The macro is what reports
a missing required prop at the line that wrote the tag rather than from inside
generated code, what routes `@test_id`, `@node_ref` and `@sizing` to the right
place, what enforces a component's child arity, and what keeps render props
unbuilt until the component calls them. Hand-written builder chains lose the
diagnostics, are invisible to the `view!` formatter that `./scripts/verify`
runs, and read nothing like the rest of the tree. The same applies to a
component you want to pass around: hand over a `Render`/`RenderFn` closure that
writes a `view!`, not a half-applied builder.

### Use fine-grained reactivity

Bind a signal or memo to a prop; do not read it while building and do not
rebuild a subtree to show a new value. A `Prop<T>` takes a plain `T`, a
`ReadSignal<T>`, or a `Memo<T>`, and the reactive forms install an effect that
writes just that one property when the value changes.

The common mistake is a list. Removing every row and building replacements on
each change throws away the nodes, their scopes, their measured text, and
whatever focus or caret lived in them, and costs time proportional to the whole
list for a one-item edit. `ForEach` keyed by item identity reconciles in place:
rows that stayed keep their nodes untouched, and only the bindings reading what
actually changed run.

```rust
<ForEach spacing=8.0 keys={items.keys()}>
    {move |id: Uuid| {
        let item = items.get(&id);
        view! { <Row item /> }
    }}
</ForEach>
```

Key by identity, never by content: a key containing the row's text changes
whenever the text does, which destroys and rebuilds the row — exactly what
`ForEach` exists to avoid. Give the row the key and let it read its own item
signal. `KeyedStore` supplies both halves, one signal per item plus the order
signal, and `reconcile` writes only the items that differ.

The same instinct applies elsewhere. Derive with `create_memo` so a property
wakes only when the derived value changes. Split a struct held in one signal
with `#[derive(Store)]` so writing one field does not wake readers of the
others; `use_theme()` returns such a store, which is why a component binds
`theme.accent.clone()` rather than the whole theme. Use `create_selector` for
"am I the selected row?" so moving a selection wakes two rows instead of all of
them. Prefer `Show` over rebuilding, `Keyed` over `Dynamic` when only part of a
value decides the shape, and `VirtualList` for a collection large enough that
building every row is the cost.

## Component layers

Beui separates mechanism, behavior, and appearance. The dependency direction is
deliberate:

| Layer | Location and public path | Responsibility |
| --- | --- | --- |
| Base | `crates/beui/src/base`; re-exported from `beui::reactive` | Retained nodes for layout, painting, visibility, focus, pointer input, scrolling, and text. |
| Unstyled | `crates/beui/src/unstyled`; `beui::unstyled` | Accessible interaction behavior composed from base components, without theme colors, typography, borders, or spacing. |
| Styled | `crates/beui/src/styled`; `beui::styled` | Application-ready controls that compose an unstyled control and paint its state with base components and `styled::theme` tokens. |

Styled components are composed out of unstyled ones, and unstyled ones are
composed out of base ones. Work at the highest layer that can express what you
need:

- **Adding a feature to the app?** Compose styled controls with base layout and
  painting primitives.
- **Need existing behavior with different appearance?** Use the unstyled
  component and paint it yourself. Do not reimplement focus, keyboard, pointer,
  touch, or accessibility handling in a styled component.
- **Need new behavior?** Add an unstyled component composed from base
  components.
- **Tempted to add a base component?** Almost always, add an unstyled one
  instead. The base layer is small on purpose — `Frame`, `List`, `Text`,
  `Scroll`, `VirtualList`, `Canvas`, `Overlay`, `Focusable`, `ClickCatcher` —
  and it stays small because most things are compositions of those. Add a base
  component only when the retained tree genuinely lacks a primitive: a new way
  to lay out, paint, or receive input that cannot be expressed by arranging the
  existing nodes. If a new concern can share `Frame`'s single-child box model,
  extend `Frame` rather than adding another pass-through node.

`unstyled::Button` shows the split. It composes `Focusable` and `ClickCatcher`,
and owns button semantics, disabled behavior, pointer and keyboard activation,
and accessibility. Its content closure receives a `ButtonHandle` of reactive
`hovered`, `active`, and `focused` state. `styled::Button` wraps it and uses
that handle to choose fills and paint a focus outline, so every visual treatment
sits on the same interaction behavior.

Pure presentation components such as styled text and cards compose base
components directly, because they have no interaction behavior to delegate.

The main base building blocks are `Row`, `Column`, `List`, `Frame`, `Text`,
`Scroll`, and `VirtualList`; `Frame` combines optional sizing, padding, fill,
outline, and visibility on one retained node. The unstyled module contains
`Button`, `Pressable`, `Toggle`, `Choice`, `Slider`, `TextInput`, `Disclosure`,
`Tree`, `Select`, `ContextMenu`, `Container`, and `Stack`. The styled
module supplies themed buttons, text styles, cards, checkboxes, switches,
choices, inputs, menus, tabs, trees, progress, scrollbars, and responsive
layout. The re-exports in `unstyled.rs` and `styled.rs` are the authoritative
lists.

## Use beui in a standalone app

The default `beui` feature is `window`, which includes the winit runner and the
wgpu renderer. A standalone app builds its document once and implements
`beui::App`:

```rust
use beui::reactive::{
    Column, Frame, build, component, create_memo, create_signal, view,
};
use beui::styled::{Button, ButtonVariant, Display, use_theme};
use beui::{App, Color32, Context, Document, NodeId, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("Counter", CounterApp::new())
}

#[component]
fn Counter() -> NodeId {
    let (count, set_count) = create_signal(0i64);
    let decrease = set_count.clone();
    let label = create_memo(move || count.get().to_string());
    let theme = use_theme();

    view! {
        <Frame color={theme.background.clone()}>
            <Column spacing=8.0>
                <Display content={label} />
                <Button
                    label="Decrease"
                    variant=ButtonVariant::Secondary
                    on_click={move || decrease.update(|value| *value -= 1)}
                />
                <Button
                    label="Increase"
                    variant=ButtonVariant::Primary
                    on_click={move || set_count.update(|value| *value += 1)}
                />
            </Column>
        </Frame>
    }
}

struct CounterApp {
    document: Document,
}

impl CounterApp {
    fn new() -> Self {
        Self {
            document: build(|| view! { <Counter /> }),
        }
    }
}

impl App for CounterApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        self.document.theme().background
    }
}
```

Run the repository examples with:

```text
cargo run -p beui --example counter
cargo run -p beui --example demo
```

Beui has three feature levels:

- No features provides the document, components, layout, input model, and
  painting output. This is enough for headless logic tests.
- `render` adds the wgpu renderer without creating a window. Embedded hosts use
  this level.
- `window` adds the desktop runner and enables `render`; it is the default.

### The inspector

Ctrl+Shift+I in a standalone beui window opens the node, accessibility, and
performance inspector. Ctrl+Shift+C enables node picking. Ctrl+Shift+F moves
keyboard focus into the panel and back out again, and Escape inside the panel
returns focus to the document, so the whole inspector is reachable without a
mouse. Its tree rows select and expand together: clicking a row, or pressing
Enter or Space on it, selects the node it lists and opens or closes its
children, and the arrow keys walk the tree.

The inspector's Sim tab converts one input device into the other, so a pointer
device can drive touch behavior and a touchscreen can drive pointer behavior.
Both run inside `Document::show`, so they work the same in a standalone window
and in a beui block editor plugin.

"Emulate touch with mouse" turns mouse presses into touch events.

"Simulate mouse with touch" turns the whole shown rectangle into a trackpad and
paints a cursor the document reacts to. One finger moves the cursor, a tap
clicks it, a tap followed by a press and drag drags with the primary button, and
two fingers scroll smoothly. The strip along the bottom holds the left, middle,
and right mouse buttons plus a keyboard toggle: a button stays held for as long
as its finger is down, another finger can work the trackpad at the same time,
and swiping up or down on the middle button scrolls a wheel tick at a time. The
keyboard toggle opens an on-screen keyboard that sends `Event::Key` and
`Event::Text`; its Shift, Ctrl, and Alt keys latch until the next key, and they
also apply to clicks, so Ctrl+Shift+I on it reopens the inspector.

## Write views

Import `view!`, `#[component]`, signals, and base components from
`beui::reactive`; styled controls come from `beui::styled` and behavior-only
ones from `beui::unstyled`.

`view!` supports three framework attributes on every tag, in their own `@`
namespace so a component can name its props whatever it likes:

- `@sizing` selects the child's `ItemSize` among its siblings in a list.
  Children are intrinsic by default; fixed children reserve a logical-point
  size, and percent children share the remaining bounded space by weight.
- `@test_id` gives a node a stable name for headless interaction tests.
- `@node_ref` fills a `NodeRef` when enclosing code genuinely needs the
  resulting `NodeId`.

Use `Frame`'s `width` and `height` props to constrain a component's own size,
and `@sizing` to describe how it participates among siblings in a `Row`,
`Column`, or `List`. `Container` and `narrower_than` provide
container-responsive state; `unstyled::Stack` and `styled::Stack` switch between
a row and a column without rebuilding their children.

### Update a document from outside its events

Component callbacks run with their `Document` installed, so signal writes from
clicks and key events need no special handling. A host-driven update happens
outside that context and must enter the document's reactive scope:

```rust
use beui::reactive::{WriteSignal, with_reactive_scope};
use beui::Document;

fn set_value(document: &mut Document, value: &WriteSignal<String>, next: String) {
    let value = value.clone();
    with_reactive_scope(document, move || value.set(next));
}
```

Keep the write handles the host needs next to its `Document`. Do not rebuild the
document to display new data.

## Use beui in a block editor plugin

A beui editor is a `#[component]` function. It implements
`block_editor_plugin::BeuiApp` and uses `block_editor_plugin::beui_plugin!`
instead of the egui `App` and `plugin!`. The type it names holds no state: the
framework builds the view once, keeps the `Document` it produced, and shows it
every frame.

```rust
#[component]
pub fn Counter(editor: block_editor_plugin::Editor) -> NodeId {
    let counter = editor.block::<CounterBlock>();
    let count = counter.project(CounterBlock::count);
    view! { ... }
}

pub struct CounterApp;

impl block_editor_plugin::BeuiApp for CounterApp {
    fn view(editor: block_editor_plugin::Editor) -> NodeId {
        view! {
            <Counter editor={editor} />
        }
    }

    fn create_block(creation: &block_editor_plugin::Creation) -> Result<Uuid, String> {
        Ok(creation.client().create_block(CounterBlock::default()).id())
    }
}

block_editor_plugin::beui_plugin!(CounterApp, "../manifest.json");
```

`Editor` is everything the instance was given: the host, the runtime's client,
the block, the view the host is showing the content through, and
`each_frame(...)` for work that is neither a block projection nor a signal. The
host supplies input, fonts, clipboard integration, rendering, and the frame
rectangle. A plugin normally depends on beui without the window runner:

```toml
beui = { path = "../../beui", default-features = false, features = ["render"] }
```

Block data reaches the view through `block-reactive`: `BlockSource::new` watches
a block, `project` and `project_keyed` derive signals from its current value, and
one `pump()` at the top of the frame — inside the document's reactive scope —
re-derives them. See the [reactive guide](reactive.md#blocks).

The counter editor under `crates/editors/counter` is the reference integration.
The [plugin editor guide](adding_a_plugin_editor.md) covers the manifest,
creation flow, host connection, and current beui plugin capability limits.

A plugin with `"creation": "Dialog"` implements `creation_view` instead, one
more `#[component]` function that the framework builds a separate document of
and shows in the host's creation dialog. It says what the dialog makes with
`creation.on_create(...)` and answers `creation.set_ready(true)` once it has been
filled in. Host services such as `BlockPicker` work there in the same way they
do from an egui creation UI, polled from `creation.each_frame(...)`.

## Develop an unstyled component

An unstyled component owns semantics and interaction, not appearance. Put it in
`crates/beui/src/unstyled/<name>.rs`, declare it in `unstyled.rs`, and re-export
the public component, handles, state readers, and supporting types there.

Compose it from base components. For an interactive control this normally means:

1. Model controlled values and transient interaction values with signals.
2. Use `Focusable` for tab order, keyboard events, activation, and focus state.
3. Use `ClickCatcher` for pointer and touch interaction.
4. Publish the correct AccessKit role and state with `component_accessibility`.
5. Give the caller a `Render<Handle>` or `RenderFn<Handle>` containing the
   reactive state needed to paint the control.
6. Return the root base node directly so component state and framework slots
   attach to the node callers receive.

A press normally reaches every `ClickCatcher` under the pointer. A control that
must win a press, or that reacts to presses outside its own rect, captures it:
`capture_presses` claims presses inside the catcher and `capture_at` claims
presses at positions its callback accepts. Before any node handles a press the
document asks the topmost nodes first, and only the captor receives it: focus
stays where it is, touch scrolling does not start, and no other catcher arms.
Paint such parts with `Painter::on_top`, which draws above the rest of the
document, or of the overlay being painted. The touch selection handles of
`unstyled::TextInput` use both.

Do not put theme colors, fixed visual spacing, typography choices, or decorative
shapes in this layer. A new skin should be able to use the unstyled control
without undoing visual decisions.

State that belongs to the component's own handlers should be captured directly.
Use `set_component_state` only when tests or host integration need to read the
state from the component's `NodeId`, and expose a focused helper such as
`toggle_checked(&Document, NodeId)`. Controlled state must listen to its prop
and report user changes through its callback; see `unstyled::Toggle` and
`unstyled::TextInput` for the established pattern.

## Develop a styled component

Put a styled component in `crates/beui/src/styled/<name>.rs`, declare it in
`styled.rs`, and re-export its public API there. An interactive styled component
wraps the matching unstyled component, supplies its accessibility label when
needed, and renders the unstyled handle with base visual primitives:

```rust
#[component]
pub fn Checkbox(label: Prop<String>, checked: Prop<bool>, on_change: Callback<bool>) -> NodeId {
    view! {
        <Toggle checked on_change={move |checked| on_change.call(checked)}>
            {move |handle: ToggleHandle| {
                view! {
                    <CheckboxFace handle label />
                }
            }}
        </Toggle>
    }
}
```

The face component derives colors and visibility with memos over
`handle.checked`, `handle.hovered`, `handle.active`, and `handle.focused`, then
composes `Frame` and `Text`. Keyboard and pointer handling stay in the unstyled
control. `styled/checkbox.rs` is a short, complete example of the pair.

Read colors from the nearest theme with `styled::use_theme()`, which returns a
`ThemeStore` — one `ReadSignal` per token. Bind a token straight to a prop with
`theme.accent.clone()`, or read tokens inside a memo that also reads interaction
state, so the control repaints when either changes. Because each token is its
own signal, a component wakes only for the colors it actually uses. Sizes,
radii, and font sizes are constants in `styled::theme`. Add a field to `Theme`,
with a value in every built-in theme, when a color is part of the theme rather
than unique to one component.

Styled controls must visibly expose keyboard focus, and must keep labels and
accessible roles stable when visual state changes. Use glyphs from `beui::icons`
with `styled::Icon` or `styled::IconSized`; do not use Unicode characters as
ad-hoc icons. The [keyboard guide](beui_keyboard.md) records the expected
behavior for each control family.

### Themes

`styled::Theme` holds the color tokens, and `Theme::DARK` and `Theme::EINK` are
the built-in themes. Every `Document` owns a theme that styled components use
when no provider covers them. Change it with `Document::set_theme` and read it
with `Document::theme`, for example to pick an app's clear color. The
inspector's Sim tab switches it at runtime.

`ThemeProvider` overrides the theme for the subtree written between its tags and
follows the `Prop<Theme>` it is given:

```rust
view! {
    <ThemeProvider theme={theme_signal}>
        <Settings />
    </ThemeProvider>
}
```

## Develop a base component

Base nodes are the only layer that should normally mutate a `Document` directly.
Put the node in `crates/beui/src/base/<name>.rs` and register the module in
`base.rs`. A base implementation has three parts:

- A crate-private node struct containing its retained state and child ids.
- An `Element` implementation for measurement, layout, painting, interaction,
  child traversal, and inspector metadata.
- `Document::create_*` and `Document::set_*` methods plus a public
  `#[component]` wrapper. The wrapper creates the node with `with_document` and
  binds reactive props to setters with `create_effect`.

Only invalidate retained state when a setter actually changes a value. A
spurious mutation invalidates layout or paint caching for the entire document.
Return children from both interaction traversal and `children`, give the node a
stable `kind` for the inspector, and add a concise `detail` when it makes the
tree easier to understand.

The `base` module itself is private. Re-export base components intended for
composition from `beui::reactive`, as the existing `Frame`, `Text`, and `Scroll`
components are, and keep implementation-only primitives crate-private when they
exist solely to support an unstyled control.

## Test and verify changes

Beui behavior is tested headlessly. For library behavior, add a test under the
relevant `tests` directory and declare it in that directory's `tests.rs`; this
repository keeps one test per file. The document tests use their `Harness` to
build a `Document`, send `Event` values through a `Context`, and inspect
component state, layout, accessibility, or painting output.

For a beui block editor, use `block_ui_test::BeuiTest`, built from an `Editor`
(or a `Creation`, for a dialog) so the test holds the same handle the component
was handed. Give every interacted node an `@test_id`, call `run` after queued
gestures, assert the resulting block state, and snapshot only when the painting is meaningful. `BeuiTest` also
supports key presses, text, hover, pointer clicks, and touch gestures. See the
[GUI testing guide](testing_a_gui.md) and the counter editor tests for examples.

From the workspace root, use:

```text
./scripts/check
./scripts/verify
```

`./scripts/check` is the fast complete-workspace compile check. Always finish a
coherent change with `./scripts/verify`; it runs the workspace tests, lints,
formatting, project structure checks, snapshot updates, and the formatter for
`view!` bodies that rustfmt cannot handle. Use a package-scoped Cargo command
only as a narrow diagnostic after one of the supported scripts has exposed a
failure. Run `./scripts/run --smoke` as well when a change can affect native
startup or runtime integration.
