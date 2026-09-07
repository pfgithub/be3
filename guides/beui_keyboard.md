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

Tab and Shift+Tab traverse visible controls in tree order and wrap within the document. Hidden panels and collapsed content are excluded. Changing a selection programmatically updates the group's Tab stop and moves focus with the selection when the group already contains focus. Programmatic changes do not pull focus from other controls. Empty groups have no Tab stop, and invalid selection updates are ignored.

## New control APIs

`styled::radio_group(document, labels, selected)` and `styled::listbox(document, labels, selected)` accept `Option<usize>` selections. Both provide matching `*_selected`, `set_*_selected`, `set_*_on_change`, and `focus_*` functions. `None` clears the selection; the first option becomes the entry point. Their change callbacks receive `Option<usize>`.

`styled::toggle_button(document, label, pressed)` provides `toggle_button_pressed`, `set_toggle_button_pressed`, `set_toggle_button_on_change`, and `focus_toggle_button`. Its label remains stable as the pressed state changes.

The demo's Choices tab shows all three controls and their change callbacks.

## Composing controls and embedding beui

Unstyled buttons expose `button_focusable`, `set_button_tab_stop`, and `set_button_on_key` for compound controls. Return `true` from a key handler only when it handles that key. `Document::set_focusable_tab_stop` removes an option from sequential Tab navigation while preserving pointer and programmatic focus. Unstyled controls expose focus-change callbacks so their owner can paint a focus indicator. `Document::focused_node` reports the current focus.

A host sends `Event::Focus(false)` when its window or editor region loses focus. Text and paste arrive through `Event::Text`. Copy and cut return text in `FrameOutput::copied_text`; the host writes this to its clipboard. Both the desktop runner and the block editor integration handle these outputs. Clipboard access for other custom hosts belongs to their platform integration.

Keyboard regression tests run without a window. Run `cargo test -p beui --lib --no-default-features` for the control and document tests, and `./scripts/verify` for the required workspace verification.
