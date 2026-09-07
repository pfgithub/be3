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

## New control APIs

`styled::radio_group(document, labels, selected)` and `styled::listbox(document, labels, selected)` accept `Option<usize>` selections. Both provide matching `*_selected`, `set_*_selected`, `set_*_on_change`, and `focus_*` functions. `None` clears the selection; the first option becomes the entry point. Their change callbacks receive `Option<usize>`.

`styled::toggle_button(document, label, pressed)` provides `toggle_button_pressed`, `set_toggle_button_pressed`, `set_toggle_button_on_change`, and `focus_toggle_button`. Its label remains stable as the pressed state changes.

The demo's Choices tab shows all three controls and their change callbacks.

`styled::select(document, options, selected)` opens a popup with a search box over the option list; it provides `select_selected`, `set_select_selected`, `select_open`, `set_select_open`, `set_select_on_change`, and `focus_select`. `styled::context_menu(document, region, items)` wraps an existing region so a secondary click opens a menu built from `unstyled::MenuItem` values (`MenuItem::new` for a leaf, `MenuItem::with_children` for a submenu); it provides `set_context_menu_items` and `set_context_menu_on_select`, whose callback receives the selected item's index path through any submenus. Both are built on a new `base` overlay primitive: an anchored, viewport-relative popup that beui did not have before, painted above the rest of the tree and traps Tab while open. The demo's Menus tab shows both.

## Composing controls and embedding beui

Unstyled buttons expose `button_focusable`, `set_button_tab_stop`, and `set_button_on_key` for compound controls. Return `true` from a key handler only when it handles that key. `Document::set_focusable_tab_stop` removes an option from sequential Tab navigation while preserving pointer and programmatic focus. Unstyled controls expose focus-change callbacks so their owner can paint a focus indicator. `Document::focused_node` reports the current focus.

`unstyled::set_text_input_on_key_override(document, input, handler)` lets a compound control (such as select's search box) intercept specific keys, such as arrows, before the text input's own key handling runs; returning `false` falls through to the text input's normal behavior.

A host sends `Event::Focus(false)` when its window or editor region loses focus. Text and paste arrive through `Event::Text`. Copy and cut return text in `FrameOutput::copied_text`; the host writes this to its clipboard. Both the desktop runner and the block editor integration handle these outputs. Clipboard access for other custom hosts belongs to their platform integration.

Keyboard regression tests run without a window. Run `cargo test -p beui --lib --no-default-features` for the control and document tests, and `./scripts/verify` for the required workspace verification.
