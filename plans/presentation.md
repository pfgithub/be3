# Presentation editor: feature recommendations

Recommendations only, not implemented. Based on the current state of
`crates/block-client/src/blocks/presentation.rs`,
`crates/block-app/src/editors/presentation.rs`, and
`crates/block-app/src/slide_templates.rs`.

Current state: a filmstrip with drag-reorder, add-slide (template or link an
existing block via the picker), detach/delete, fullscreen playback with
arrow/space/home/end navigation and auto-hiding controls, and three canvas
templates (Title/Regular/Blank). Slides are just `{id, block_id}` — no other
per-slide metadata exists yet.

## Presenting

- **Speaker notes per slide** — biggest gap. Needs a new field on
  `PresentationSlide`. Default to linking a `text` block for the notes, but
  let the user replace it with any block type they choose, same as slides
  themselves are just a block reference.
- **Presenter view** — separate window/second-monitor mode showing current
  slide, next slide thumbnail, notes, and an elapsed timer.
  `show_playback_surface` already isolates the render logic, so this is
  mostly a second `egui::Area`/viewport reusing it.
- **Jump to slide by number** (type digits + Enter) and **blank/black screen
  toggle** (`B`/`.` like PowerPoint) — cheap additions to the existing
  key-handling block in `show_playback`.
- **Live "follow presenter" mode** — fits this codebase well since presence
  and block sync are already first-class: broadcast the presenter's current
  slide via presence so viewers' clients auto-advance with them. Worth
  prioritizing given how little new infra it needs.
- Auto-advance/timed playback, loop back to slide 1 at the end.

## Slide management

- **Duplicate slide** — currently there's no way to clone a slide's content;
  `insert_slide` only links an existing block. This should be a shallow
  copy: clone the `InfiniteCanvas` itself, but any sub-blocks referenced from
  inside it (e.g. an embedded canvas entity pointing at another block) stay
  linked to the same underlying block, with no special deep-copy behavior.
- **Multi-select** (shift/ctrl-click in the filmstrip) for bulk delete/move.
- **Reorder via keyboard** (move up/down) as an alternative to drag-and-drop.
- **Section dividers / grouping** for long decks — filmstrip is a flat list
  today.
- **Grid/overview mode** — a multi-column thumbnail grid instead of the
  single vertical filmstrip, useful once a deck has 20+ slides.

## Content/theming

- **More templates**: two-column, image+caption, section header/agenda.
- **Per-slide transition** (cut/fade) during playback.
- Shared theme / master slide: skipped. Depends on infrastructure work that
  hasn't been done yet (there's no concept of a deck-level style that
  propagates to independently-owned `InfiniteCanvas` slides).

## Export/sharing

- Export to PDF or images, via `dynamic_artifact` (the same pattern
  `pixel_art` and `gui_builder` use to export generated output).
- Print/handout view: skipped.

## Priority

Start with speaker notes, duplicate slide, and follow-presenter mode — they
close the biggest functional gaps and follow-presenter leans directly on
infra that already exists.

## Filmstrip while a slide is being edited

Frame ownership replaces the chrome the deck used to carve out of its own
regions: while a slide is selected the slide owns the toolbar row and both
sidebars, so the deck's filmstrip is hidden until Escape hands the frame back.
That is an accepted regression for now. The end state that undoes it is the
filmstrip moving out of the deck's left sidebar and into the deck's content
area, alongside the stage, where a slide taking the chrome bands cannot hide
it.
