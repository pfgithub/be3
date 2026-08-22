# Plugin subeditors

Every editor becomes a plugin eventually, and the first ones with subeditors are
presentation, then infinite canvas, then text. All three embed live editors that
may themselves be plugins, several levels deep. This proposes how a plugin
embeds another block's editor, and folds the existing top-level plugin rendering
into the same mechanism.

## Cut-through composition

A plugin renders its region into its runtime's atlas, which is cleared to
transparent, and the host composites that region as one alpha-blended quad. So a
child is drawn *under* the parent's quad and the parent cuts a hole in itself
where the child shows through.

Ordering per region is `children (deepest first) -> parent quad`. Everything the
parent draws over a child is above it because it lives in the parent's texture,
which is on top: selection handles, drag ghosts, marquees, menus, a modal over a
slide. No extra screens, no extra egui contexts, no layer concept for the plugin
author.

The hole is cut by a paint callback that the child widget inserts into the
parent's own egui shape stream. egui_wgpu runs callbacks in painter order, so the
callback erases the parent's paint before it and anything painted after re-covers
the hole. Parent z-order is ordinary egui painter order. The punch is one wgpu
pipeline in `panes.rs`, shared by Linux, Windows and web. The parent chooses the
hole's shape, so a rounded child keeps the parent's background around its
corners.

A child that is genuinely translucent cannot composite against parent content
this way, so each child carries a layer flag: `Below` (hole, parent may overlay -
the default, and what all three migrations want) or `Above` (no hole, composites
against the parent's pixels, parent cannot overlay). The host emits
`children below -> parent quad -> children above`. Interleaving more than these
two parent layers is out of scope.

## Input routing

Visual occlusion and input occlusion come from one declaration or they drift.
With each frame a runtime publishes, per region, an ordered list of input holes
(`child_id`, block id and type, rect, clip, layer, mode) and the occluder rects
declared after each child. A child's interactive area is its rect minus the
occluders that follow it. Occluders are explicit (`host.occlude(rect)`) and
automatic for egui areas - menus, popups, tooltips.

The default is that the parent wins; a hole is the exception. A bug routes an
event to the parent, never invisibly through to a child.

The host resolves an event against the last published table when the event
arrives and hands it straight to the owning runtime, so every level receives its
input in parallel before any of them draws, instead of the event walking down the
tree one frame per level. The table is last-frame data, exactly like egui's own
hit-testing, and it matches the pixels on screen, which are also last frame.

A runtime only ever receives events for areas it owns. A child stays `Passive`
(nothing routed to it; the parent sees the clicks, so canvas selection and
dragging keep working) until the parent's own gesture promotes it to `Active`,
which is what `focused_editor` and `focused_embed` already do today. Escape is
host-owned - click outside, Esc - so a parent cannot trap input, and the parent
can revoke by republishing the child as passive. A hostile parent can lie about
geometry, which is annoying rather than dangerous: it cannot read a child's
pixels or its events.

## The top-level plugin is a child too

The host stops drawing plugin regions its own way. A plugin editor opened in a
tab is a depth-0 child: the host publishes a placement for it, composites it in
the same pass, and routes its input through the same table. `editor_ui` no longer
allocates a painter, forwards every event it received and presents inline;
instead it reports the placement and returns a deferred present handle that the
caller emits after the children below it.

That gives one composition and one routing path for every depth, so a plugin
nested three levels down behaves like one in a tab, and host-native parents can
declare holes and occluders through the same call while they are still native -
which is what lets presentation, infinite canvas and text migrate one at a time
instead of all at once.

## Depth

Runtimes are keyed by plugin id, so presentation -> canvas -> text -> image is
four surfaces and four quads, while a canvas inside a canvas is one runtime with
two instances. Each level publishes placements in its own region coordinates and
the host maps them through the parent's placement recursively, so nested holes
and clips fall out of the recursion, as does drawing innermost-first.

Host-side this reuses what exists: `editors.ensure`, the active-path recursion
guard in `EditorAccess::with_editor`, and the access ceiling in `access_for`. A
child already on the active path is refused and the parent is told so it can draw
its own fallback.

## Two things to get right

**Placement and pixel lockstep.** Placements are published with the frame
generation and the host draws children at the placements belonging to the frame
it is presenting. With cut-through this is not cosmetic: a mismatch shows host
background through the hole.

**Cross-level pacing.** While a canvas zooms, a child's last frame was rendered
for a different rect. Stretch that frame into the new hole - cheap and the same
degradation a resized surface already shows. Keep the generation in the placement
message so a composition barrier, where the host holds a parent frame until its
children have produced one for that generation, can be added per region later.

Smaller: every visible nested instance needs a screen in its runtime's atlas, so
parents declare only visible children and the host needs a budget that falls back
to preview rendering when a canvas has hundreds of embeds; drag-and-drop is
delivered innermost-first with `accept_drag` as the bubbling signal; keyboard and
IME go to the deepest active screen and the cursor comes from the deepest hovered
owner.

When a child repaints, its parents do not: the hole is unchanged and the host
recomposites. Handing rendered child textures back to a parent instead would
force a re-render and a hand-off at every level above the change.

## Work

1. `block-plugin-api`: `ChildPlacements { instance, region, generation, children,
   occluders }` up, `ChildStatus { available, intrinsic_size, aspect_ratio,
   hovered, active, error }` down. Validate coverage, round-trip test,
   `PROTOCOL_VERSION`, `PROTOCOL.md`.
2. `block-editor-plugin`: `host.child(ui, block_id, block_type) -> ChildHandle`,
   `host.occlude(rect)`, and the punch pipeline in `panes.rs`.
3. `plugin_host/instances.rs`: placements per screen keyed by generation, expired
   on the existing `last_seen` pass idiom, exposed to the input path.
4. `plugin_host/input.rs`: resolve events against the table before dispatch,
   deliver per runtime, keyboard and IME to the deepest active screen.
5. `plugin_host` + `editors/plugin.rs`: deferred present handle, top-level plugin
   regions published as depth-0 children, `editors.ensure` plus
   `embedded_editor_ui` or `render` per child, `EditorAction` propagation.
6. `EditorHost::pick_block(filter)` mirroring `pick_file` and reusing
   `BlockPicker`, otherwise a plugin parent can only acquire children by
   drag-and-drop.

Staged: top-level regions moved onto the new path first (no children yet, so it
is a pure refactor with the old behaviour), then preview-mode children, then
passive children with parent-owned focus, then active children with full routing,
then `pick_block`.

Presentation migrates first: rectangular, non-overlapping, mostly passive
children, and overlays that are exactly what cut-through fixes. Canvas then
exercises zoom-time pacing, and text exercises children inside a scrolling flow.
