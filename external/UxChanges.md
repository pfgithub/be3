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
- [x] Add zoom presets, 100%, fit-all, and fit-selection controls.
- [x] Add a shortcut/help popover and empty-canvas onboarding instructions.
- [x] Add tool shortcuts (`V`, `R`, `L`, `T`, `P`), Select All, Invert Selection, arrow-key nudging, Shift-arrow large nudging, duplicate, Enter-to-edit, Escape-to-select, and Ctrl/Cmd+`[` / `]` layer shortcuts.

## Selection and transformation

- [x] Constrain line drawing and rotation while Shift is held, with 15-degree rotation snapping.
- [x] Add exact position, dimensions, and rotation fields to a reorganized inspector.
- [x] Add group and ungroup operations and persistence.
- [x] Add lock and unlock operations and persistence; locked objects must not move, resize, rotate, or delete accidentally.
- [x] Add Alt-drag duplication.
- [x] Use rotation-aligned selection bounds and handles for a single rotated selection.
- [x] Add multi-selection alignment and distribution controls.

## Text editing

- [x] Extend canvas text data with font size, weight, alignment, line height, wrapping, and auto-size behavior.
- [x] Create auto-width text by clicking and wrapped text boxes by dragging, starting empty with inline focus.
- [x] Add multiline inline canvas editing; double-click or Enter edits, Escape commits, and Ctrl/Cmd+Enter exits.
- [x] Make text resize wrap by default and scale when Alt is held.
- [x] Add text typography and layout controls to the inspector and remove the toolbar text field.

## Commands, menus, and inspector polish

- [x] Centralize canvas selection commands for reuse by keyboard, toolbar, and context menus.
- [x] Add duplicate, delete, cut, copy, paste, lock, group, open/edit, preview/direct conversion, fit selection, select all, and invert selection to the appropriate context menus.
- [x] Make the Add context menu match the toolbar, including text, pen/freehand, image, and blocks.
- [x] Add a compact toolbar Actions menu for common selection and clipboard commands.
- [x] Reorganize the inspector into collapsible Transform, Appearance/type-specific, Arrange, and Block sections with object-type labels and compact layer controls.
- [x] Disable layer actions that cannot change the current order and use consistent Forward/Backward wording.
- [x] Coalesce each inspector slider or numeric drag into one undo history step and close the group on pointer release.
- [x] Support copying, cutting, and pasting canvas entities with fresh entity/group IDs and safe offsets.

## Final verification

- [x] Run final `cargo fmt`.
- [x] Run final `cargo nextest run`.
- [x] Confirm every implementation item has its own successful commit and the worktree is clean.

## Follow-up fixes

- [x] Do not select an outlined rectangle when an empty click lands inside its bounds.
- [x] Leave lines, rectangles, and freehand strokes unselected after creating them.
- [x] Restore double-click inline editing for canvas text.
- [x] Disable Group when every selected object is already in the same group.

## Follow-up verification

- [x] Run `cargo fmt` and `cargo nextest run` after every follow-up fix.
- [x] Confirm every follow-up item has its own successful commit and the worktree is clean.

## Additional interaction changes

- [x] Require a marquee selection to contain an entire object before selecting it.
- [x] Let Ctrl move the starting point and Alt use it as the center while box-selecting or drawing rectangles.
- [x] Edit canvas text in the inspector, focusing it after placement or a double-click.
- [x] Show the active non-selection tool icon beside the pointer.

## Additional verification

- [x] Run `cargo fmt` and `cargo nextest run` after every additional change.
- [x] Confirm every additional item has its own successful commit and the worktree is clean.

## Direct editor interaction changes

- [x] Make the zoom percentage reset to 100% and move preset choices into a separate menu button.
- [x] Keep database, text, and presentation direct editors live; require selection and an Edit button for preview-based editors.
- [x] Move direct-editor scale into the inspector and make resize behavior capability-driven.
- [x] Render embedded presentations as resizable fullscreen playback with slide navigation; require opening the block to edit.
- [ ] Allow embedded text direct editors to resize horizontally.

## Direct editor verification

- [ ] Run `cargo fmt` and `cargo nextest run` after every direct-editor change.
- [ ] Confirm every direct-editor item has its own successful commit and the worktree is clean.
