# beui controls and keyboard behavior

The styled controls follow the keyboard conventions in the [W3C Authoring Practices Guide](https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/). They paint focus outlines, and pointer selection and keyboard selection share the same state and change callbacks.

| Control | Keyboard behavior |
| --- | --- |
| Buttons, toggle buttons, checkboxes, switches, list rows, disclosure and accordion headers | Space or Enter activates on release. Escape or focus loss cancels a held activation. Repeated key-down events do not activate repeatedly. |
| Tabs | One Tab stop, at the selected tab. Left/Right wrap and select immediately. Home/End select the first/last tab. Up/Down leave the horizontal tab selection alone. |
| Radio groups | One Tab stop, at the selected option or the first option when unselected. Arrows wrap and select; Space selects without clearing an existing selection. Home/End select the first/last option. |
| Single-select listboxes | One Tab stop. Up/Down select the previous/next option and stop at the ends. Home/End select the first/last option. Typing searches case-insensitive prefixes; repeated letters cycle matches. The search resets after one second or when focus leaves. |
| Sliders | Right/Up increase and Left/Down decrease by 5%. Home/End select minimum/maximum. Page Up/Down adjust by 20%. Values remain within the range. |
| Text inputs | Left/Right, Home/End, Shift-selection, Ctrl/Alt word navigation and deletion, Ctrl+A, Ctrl+C/X, Ctrl+Z, Ctrl+Shift+Z/Ctrl+Y, and Enter to submit. Space inserts text. Paste replaces the selection. The desktop runner maps Command to Ctrl on macOS. |
| Scroll areas | Tab focuses the area. Up/Down scroll by a line; Page Up/Down and Space/Shift+Space scroll by a page; Home/End reach the endpoints. Tabbing to a child or navigating a choice scrolls it into view. Unused Up/Down, Home/End, and Page keys on child controls scroll the nearest containing area. Virtual lists can be paged before tabbing into their realized controls. |
| Select (dropdown) | Clicking or activating the trigger opens the popup and focuses its search box; typing filters the options by case-insensitive substring. Up/Down/Home/End on the closed trigger also open the popup and move the highlight in that direction. Up/Down move the highlighted option without moving the text caret; Home/End jump to the first/last visible option. Enter confirms the highlighted option and closes the popup. Escape or an outside click closes the popup without changing the selection and returns focus to the trigger. |
| Context menu | Secondary click opens the menu at the pointer and focuses its first item, which is shown with a highlighted background; Tab is trapped on the menu's single roving Tab stop while it is open. Up/Down move between items and update the highlight; Home/End jump to the first/last item. Right Arrow (or hovering an item) opens its submenu and focuses its first item; Left Arrow closes a submenu and refocuses the item that opened it. Only one submenu per level stays open. Enter or clicking a leaf item selects it and closes the entire menu stack; Escape closes one level at a time; an outside click closes the whole stack. |

Tab and Shift+Tab traverse visible controls in tree order and wrap within the document. Hidden panels and collapsed content are excluded. Changing a selection programmatically updates the group's Tab stop and moves focus with the selection when the group already contains focus. Programmatic changes do not pull focus from other controls. Empty groups have no Tab stop, and invalid selection updates are ignored.

## Control props

Every control is a `#[component]`, so it is written as a tag inside `view!` and
driven by reactive props rather than by setter calls. `<radio_group>` and
`<listbox>` take `labels` and an `Option<usize>` `selected` prop and report
changes through `on_change`; `None` clears the selection, and the first option
becomes the entry point. `<toggle_button>` takes `label` and `pressed`, and its
label stays stable as the pressed state changes. The demo's Choices tab shows
all three.

`<select>` takes `options` and `selected` and opens a popup with a search box
over the option list. `<context_menu>` wraps a `region` so a secondary click
opens a menu built from the `items` prop, a `Vec<unstyled::MenuItem>`
(`MenuItem::new` for a leaf, `MenuItem::with_children` for a submenu); its
`on_select` callback receives the selected item's index path through any
submenus. Changing `items` rebuilds the menu. Both controls sit on the `base`
overlay element: an anchored, viewport-relative popup painted above the rest of
the tree that traps Tab while open. The demo's Menus tab shows both.

Reading a control's state back out, rather than owning the signal that drives
it, is for tests and host integration: `*_selected`, `*_open`, `*_pressed`,
`slider_value`, `text_input_value` and friends take `&Document` and a node.
`focus_*` takes just the node and focuses the control ambiently.

## Composing controls and embedding beui

The unstyled controls hand their interaction state to whoever renders their
content: `<unstyled::button content={...}>` calls the content builder with a
`ButtonHandle` carrying `hovered`, `active` and `focused` signals, and the
text input and select equivalents do the same. Build reactive props out of
those signals — including effects that react to a child's hover or focus —
instead of querying the control's state back afterwards.

`tab_stop` is a prop on `<unstyled::button>` and `<focusable>`: set it to
`false` to keep a control reachable by pointer and programmatic focus while
removing it from sequential Tab navigation, which is how single-Tab-stop groups
work. `on_key` returns `true` only for keys it handled. `<unstyled::text_input>`
also takes `on_key_override`, which lets a compound control (such as select's
search box) intercept arrows before the text input's own key handling runs;
returning `false` falls through to the normal behavior. `Document::focused_node`
reports the current focus.

Use `@node_ref=&a_node_ref` on any tag when an enclosing component needs the
`NodeId` of something nested inside its tree; `NodeRef::get` reads it back once
the tree is built.

A host sends `Event::Focus(false)` when its window or editor region loses focus. Text and paste arrive through `Event::Text`. Copy and cut return text in `FrameOutput::copied_text`; the host writes this to its clipboard. Both the desktop runner and the block editor integration handle these outputs. Clipboard access for other custom hosts belongs to their platform integration.

Keyboard regression tests run without a window. Run `cargo test -p beui --lib --no-default-features` for the control and document tests, and `./scripts/verify` for the required workspace verification.

## Touch behavior

BEUI accepts `Event::Touch` with a stable device and finger id, lifecycle
phase, logical position, and optional normalized pressure. It tracks every
active contact in `InputState::touch`; the first contact drives the primary
pointer so existing pressable controls, sliders, text selection, focus, and
overlays work without a separate touch-only control API. A second contact or
a cancelled contact cancels a pending tap rather than activating it.

A tap may drift by up to eight logical points. Beyond that threshold BEUI
locks the gesture to its dominant axis. Vertical gestures drag the deepest
scroll view under the initial contact, keep that scroll captured when the
finger leaves its rectangle, and do not click the row where the gesture
started. A released vertical drag coasts with its sampled velocity, and a drag
beyond either end of a scroll view is resisted before springing back. Touch
contacts do not hover controls, and focus is assigned only after a gesture
resolves as a tap. Horizontal gestures remain available to controls such as
sliders. The touch pointer disappears after release, so touch does not leave
hover styling behind. Platform integrations may provide synthesized pointer
events alongside touch events; BEUI suppresses those duplicates while the
touch is active.

Standalone apps receive winit touch events automatically. Open the inspector
with Ctrl+Shift+I and enable “Emulate touch with mouse” to turn the primary
mouse button into a touch contact. Embedded BEUI plugins receive the same touch
data over the block plugin input protocol. `block_ui_test::BeuiTest` provides
`touch_start`, `touch_move`, `touch_end`, and `touch_cancel` for headless
gesture tests.
