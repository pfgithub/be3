# Infinite Canvas UX Changes

Each implementation item is checked only after `cargo fmt`, `cargo nextest run`, and its dedicated commit succeed.

## Placement and interaction fixes

- [x] Place toolbar-created blocks, linked blocks, and file-picker images at the visible viewport center.
- [x] Hit-test unfilled rectangles by their outline instead of their interior.
- [x] Enter a direct editor only from its content area; use its padding and title bar for canvas selection and movement without clearing an existing multi-selection unnecessarily.
- [x] Split canvas and selection context menus; choose the selection menu from the selected bounds and only object-hit-test when nothing is selected.
- [x] Stop unconditional idle repainting while retaining repaint requests for active/asynchronous interactions.

## Navigation and discoverability

- [x] Make ordinary wheel/trackpad scrolling pan, Ctrl/Cmd-wheel and pinch zoom, and retain middle-drag and Space-drag panning.
- [x] Add appropriate hand, resize, rotate, crosshair, text, and pen cursors.
- [ ] Add zoom presets, 100%, fit-all, and fit-selection controls.
- [ ] Add a shortcut/help popover and empty-canvas onboarding instructions.
- [ ] Add tool shortcuts (`V`, `R`, `L`, `T`, `P`), Select All, Invert Selection, arrow-key nudging, Shift-arrow large nudging, duplicate, Enter-to-edit, Escape-to-select, and Ctrl/Cmd+`[` / `]` layer shortcuts.

## Selection and transformation

- [ ] Constrain line drawing and rotation while Shift is held, with 15-degree rotation snapping.
- [ ] Add exact position, dimensions, and rotation fields to a reorganized inspector.
- [ ] Add group and ungroup operations and persistence.
- [ ] Add lock and unlock operations and persistence; locked objects must not move, resize, rotate, or delete accidentally.
- [ ] Add Alt-drag duplication.
- [ ] Use rotation-aligned selection bounds and handles for a single rotated selection.
- [ ] Add multi-selection alignment and distribution controls.

## Text editing

- [ ] Extend canvas text data with font size, weight, alignment, line height, wrapping, and auto-size behavior.
- [ ] Create auto-width text by clicking and wrapped text boxes by dragging, starting empty with inline focus.
- [ ] Add multiline inline canvas editing; double-click or Enter edits, Escape commits, and Ctrl/Cmd+Enter exits.
- [ ] Make text resize wrap by default and scale when the modifier is held.
- [ ] Add text typography and layout controls to the inspector and remove the toolbar text field.

## Commands, menus, and inspector polish

- [ ] Centralize canvas commands used by keyboard, toolbar, and context menus.
- [ ] Add duplicate, delete, cut, copy, paste, lock, group, open/edit block, preview/direct conversion, fit selection, select all, and invert selection to the appropriate context menus.
- [ ] Make the Add context menu match the toolbar, including text, pen/freehand, image, and blocks.
- [ ] Add visible toolbar actions for common selection commands without crowding the drawing tools.
- [ ] Reorganize the inspector into Transform, Appearance, type-specific, Arrange, and Block sections with object-type labels and compact layer controls.
- [ ] Disable layer actions that cannot change the current order and use consistent Forward/Backward wording.
- [ ] Coalesce each inspector slider or drag into one undo history step.
- [ ] Support copying and pasting canvas entities, including fresh IDs and safe offsets for duplicates.

## Final verification

- [ ] Run final `cargo fmt`.
- [ ] Run final `cargo nextest run`.
- [ ] Confirm every implementation item has its own successful commit and the worktree is clean.
