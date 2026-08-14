# Version control proposal

Git-shaped version control built on top of BE3's existing block primitives
(parent/child tree, `references()`, `BlockAccess` sharing) rather than as a
bolt-on system. High-level design only — no code.

## Goals (from the original ask)

- A `version_control_data` block holds branches (starting with `main`) and an
  append-only commit history. Creating one creates `main` pointing at an
  empty initial commit.
- A `version_control_worktree` block references a data block and holds a
  checked-out commit. Creating one against a fresh data block starts empty.
- Inside a worktree you create ordinary blocks (folders, files, etc.) the
  normal way. Committing snapshots the worktree's current content onto its
  tracked branch in the data block.
- Two collaboration modes, both reusing existing sharing (`BlockAccess`)
  unchanged:
  - Share the **data block** → collaborator makes their own worktree,
    works independently, shares commit history only.
  - Share a **worktree block** → collaborator gets live, real-time editing
    of your current uncommitted content (access already flows down from
    parent to child in BE3 today).

## Core model

- **Object store** (`version_control_object`): a *blob* (`{source block
  type, serialized state}`) or a *tree* (`{entries: [(eternal_id, kind,
  content_hash, name)]}`), immutable once created. `NoHistory`, `CRDT =
  true` (a commit can fire off many object creates without waiting on a
  round-trip ack per one). Objects get ordinary, randomly-generated live
  IDs — `content_hash` is only a lookup key, not a deterministic ID.
  Before creating a new object, a commit checks *this repo's own* hash →
  object index, built by walking this `version_control_data`'s own commit
  history (dedup is per-repo; a commit never looks at another repo's
  objects, so there's no cross-workspace access question). Two commits
  racing on identical new content can each end up creating an object for
  the same hash — harmless: the loser is simply never pointed at by any
  tree, ends up with no backrefs, and sits orphaned (no block-deletion
  primitive exists to reclaim it, same as everywhere else in BE3 today).
- **Commit**: `{parent, tree hash, author, time, message}`, stored on
  `version_control_data`. Pure additive/content-addressed, so trivially
  conflict-free.
- **Branch**: name → commit id, stored on `version_control_data`. The one
  genuinely contested operation: advancing a branch is compare-and-swap
  (reject if it moved since you last read it — fast-forward only for v1,
  no auto-merge; a rejected commit means you reconcile by hand, e.g. a new
  branch).
- **Eternal IDs**: every tree entry has a permanent `eternal_id`, assigned
  by a worktree the moment a block becomes a member (created in it, or
  dragged in) — not derived from name/position, so renames don't lose
  identity. Each worktree keeps its own `eternal_id ↔ live_id` map as part
  of its state. A member's real `BlockParent` (a live id pointing at its
  container) is never itself stored in the vcs — containment is expressed
  purely by tree nesting via `eternal_id`s, and re-derived as a live
  `BlockParent` on materialization.
- **No dedicated folder block needed** — `WorkspaceIndex` (`editors/
  workspace_index.rs`, `DISPLAY_NAME: "Folder"`) already is the generic
  container block used everywhere else in the app; worktree content just
  uses it normally.

## Cross-block references inside a repo

Blocks that reference other blocks (13 types override `references()`
today: `text`, `database`, `database_view`, `infinite_canvas`,
`compiled_logic`, `presentation`, `hotbar`, `logic_game`, `logic_grid`,
`map`, `settings`, `video`, `workspace_index`) need a reference form that
survives checkout unchanged:

- **Direct(Uuid)** — a real, permanent live block ID. Behaves exactly as
  today; included in `references()`, validated/backref'd by the server.
- **Repo-relative { repo, eternal_id }** — used when the target is a
  member of the *same* worktree as the referencing block. Never rewritten,
  by any commit or checkout — this is what guarantees two worktrees of the
  same commit are byte-identical (e.g. copying a text block's content into
  an external editor). Excluded from `references()` — the server never
  needs to know about it. Resolved to a live ID only at point of use
  (opening, rendering, simulating), via a shared `BlockClient` helper that
  walks the referencing block's `BlockParent` ancestry to find its
  worktree and looks up `eternal_id` in that worktree's map. Unresolvable
  → rendered as a broken link, same as a deleted file.

A second helper (`classify`) runs whenever a reference is created (user
picks a target block): same ancestry walk, decide Direct vs Repo-relative,
minting the target's `eternal_id` if needed.

Because references are never rewritten, **materializing a worktree is
always verbatim**: reuse an entry untouched if its content hash didn't
change, otherwise `CreateBlock` with the blob's bytes byte-for-byte, using
a freshly generated live ID. No remap pass, no ordering dependency between
entries.

## Checkout

Switching an existing worktree to a different commit/branch (not just at
creation):

- Requires a clean worktree first (live tree hashes match
  `checked_out_commit`'s tree); if dirty, block the switch and require an
  explicit "discard uncommitted changes" confirmation.
- Per entry: unchanged hash → leave the live block alone; changed →
  detach the stale live block (set its `BlockParent` to `Orphaned` — BE3
  has no block-deletion primitive yet, so this is the closest analog, and
  a natural precursor to one) and recreate verbatim from the target blob
  with a fresh live ID; entry newly present in target → create it; live
  entry absent from target → detach it the same way.
- A changed entry's live block ID does *not* survive the checkout — same
  as git not preserving file identity across checkouts. Everything that
  references it (Repo-relative, resolved live) picks up the new ID
  automatically next time it's resolved.

## Suggested units of work

Roughly in dependency order; each should be independently reviewable and
verifiable.

1. **Reference classification primitive** — `BlockRef`-shaped value
   (Direct vs Repo-relative), plus `BlockClient::classify_reference` /
   `resolve_reference` helpers walking `BlockParent` ancestry. No block
   type changes yet; this is pure infrastructure other units depend on.
2. **`version_control_object` block** — content-addressed blob/tree store,
   dedup, `NoHistory`, `CRDT = true`. Testable in isolation (create,
   read, hash-based reuse) without any worktree/data block existing yet.
3. **`version_control_data` block** — branch map + commit DAG, CAS branch
   advancement, initial `main` + empty commit on creation.
4. **`version_control_worktree` block** — references data block, tracks
   `{branch/commit, eternal_id ↔ live_id map}`; creation flow (empty
   worktree checked out at the initial commit).
5. **Commit operation** (editor-level, not a new block type) — walk live
   tree, dedupe against parent commit, build new tree/commit objects,
   CAS-advance the branch, update `checked_out_commit`.
6. **Worktree materialization / checkout** — verbatim create-or-reuse walk
   described above, including the dirty-check and discard confirmation.
   Depends on units 1–5.
7. **Migrate `text` references to Direct/Repo-relative** — smallest of the
   13 reference-bearing types (references are just substrings in a byte
   blob), and the type the byte-identity requirement was specifically
   raised for. Good first real consumer of unit 1's primitive, proves the
   pattern before the rest.
8. **Migrate remaining 12 reference-bearing types** — `database`,
   `database_view`, `infinite_canvas`, `compiled_logic`, `presentation`,
   `hotbar`, `logic_game`, `logic_grid`, `map`, `settings`, `video`,
   `workspace_index`. Same mechanical pattern per type (reference field(s)
   become `BlockRef`, operations updated, editor call sites route through
   `resolve_reference`); can be split further into one PR per type or
   batched, since they don't depend on each other.
9. **`version_control_data` editor** — branch list, commit log/graph view.
10. **`version_control_worktree` editor additions** — status/dirty
    indicator, commit button + message field, branch switch/checkout UI,
    discard-changes action, layered on top of the worktree's normal
    folder-browsing view.

Units 1–6 deliver a working, git-like repo/worktree/commit/checkout flow
usable with any block type (just without the byte-identical-reference
guarantee outside of already-Direct references). Units 7–8 are what
actually make cross-references inside a repo checkout-stable; they can
land incrementally afterward without blocking the core flow.
