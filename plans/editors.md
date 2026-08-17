# Plugin-based block editors

## Proposal

### Goals

- Make the plugin boundary the only supported way to implement a block editor. `block-app` should be an editor host, while every first-party editor is built, packaged, discovered, and run through the same public API available to third-party editors.
- Keep synchronized block models in the trusted host. A plugin edits a host-owned block through serialized snapshots and operations; it does not connect to the server, own a `BlockClient`, implement permissions, or duplicate undo/redo, presence, relationship, and dynamic-artifact rules.
- Preserve the behavior of the current `BlockEditor` surface: creation, tabs, embedded live editors, passive previews, top and side regions, intrinsic sizing, pan and zoom, access control, child blocks, navigation actions, presence, and dynamic artifacts.
- Isolate editor failures. A malformed message, panic, timeout, or crash should close only the affected editor instance and show a recoverable error in its place.
- Support desktop, web, and Android before the in-process editor path is removed. Every supported target must use its optimized GPU texture-sharing mechanism; pixel buffers and software presentation are not part of the architecture.
- Turn `crates/plugin-demo` into a small reference editor and conformance fixture built on the production SDK, not a parallel debug-only implementation.

### Architectural boundary

Block state and synchronization remain in `block`, `block-client`, and the server. The host opens the typed `BlockHandle`, enforces the effective `BlockAccess`, applies operations, owns history, resolves references, and publishes presence. Editor plugins receive only the data and capabilities granted to their instance. This avoids putting credentials and workspace keys in child processes and prevents plugins from bypassing access checks.

Move the type-erased parts of `BlockEditor`, `EditorKind`, `CreationOptions`, and `DynamicArtifactSupport` out of `block-app` and express them as a versioned contract in `block-plugin-api`. The wire representation should use UUIDs, bounded byte payloads, and plain geometry rather than Rust trait objects or `egui` types. The host retains all application chrome and translates plugin results into existing app actions.

Each plugin package has a manifest containing a stable plugin ID and version, the supported block type UUIDs, display names and Material icon names, creation mode, editor capabilities, required protocol capabilities, and target-specific entry points. One package may register multiple block types, but registration conflicts are errors and first-party packages are selected explicitly rather than by load order. The workspace build produces a catalog consumed by native, web, and Android packaging; runtime directory scanning is an additional desktop source, not the source of truth for built-in editors.

### Protocol changes

Replace the demo-specific lifecycle with an instance-oriented protocol. After the existing version and capability handshake, the host can create and destroy many editor instances in one plugin runtime. Every instance is keyed by an opaque instance ID and has a role such as direct editor, preview, creation options, or dynamic-artifact settings. Messages for an unknown or closed instance are rejected.

Add bounded messages for:

- editor discovery and registration metadata, checked against the package manifest;
- opening an instance with block ID, block type, effective access, properties, confirmed serialized state, and editor role;
- replacing the confirmed state and properties when the host observes local acknowledgements or remote changes;
- submitting a serialized block operation, finishing a history group, and requesting undo or redo;
- reporting operation acceptance or rejection and the resulting state revision;
- reading and writing typed presence payloads and receiving other clients' presence snapshots;
- watching and unwatching reference lists, receiving reference metadata, and requesting that a referenced editor be opened, embedded, or previewed by the host;
- creation requests and results, child-block actions, navigation actions, intrinsic-size changes, viewport commands, and dynamic-artifact generation results;
- named render regions for main content, top bar, left sidebar, right sidebar, preview, creation options, and artifact settings.

State messages carry monotonically increasing host revisions. Operation requests state the revision they were authored against and are acknowledged with the revision that includes them. While an operation is outstanding, the plugin may optimistically update its own UI, but the host remains authoritative and can replace the snapshot after a rejection or concurrent change. The host validates the operation by decoding it through the registered block type before calling `BlockHandle::operate`; opaque bytes are never forwarded directly to the server.

The protocol must expose capabilities rather than assume every editor implements every feature. Unsupported optional regions and actions retain the defaults currently provided by `BlockEditor`. Protocol limits should be defined per payload class: block snapshots, media-backed blocks, operation batches, presence, and frames cannot all share the demo's current one-megabyte frame limit.

### Host runtime

Promote the reusable code under `block-app/src/debug/plugin_demo` into a platform-neutral plugin host. Separate transport, session state, process/WASM lifecycle, input translation, surface presentation, and editor adaptation. `PluginEditor` implements the existing host-side `BlockEditor` during migration and maps its methods to plugin instances and named render regions. This adapter lets plugin and in-process editors coexist until the final cutover.

The host owns a plugin manager shared by all tabs and embedded editors. It discovers packages, validates manifests, starts at most one runtime per package when practical, routes instance messages, restarts crashed first-party runtimes, and shuts runtimes down when their last instance closes. Requests must be asynchronous: no editor draw call may block on process I/O. Each region displays its most recent frame, queues coalescible input and resize events, and requests repaint when a new frame or host event arrives.

Access is enforced twice. The host never sends editable capabilities to a view-only instance, and it rejects mutation messages unless the current access ceiling allows them. When access or dynamic-artifact status changes, the host updates the instance immediately. A plugin cannot request arbitrary blocks; it can only act on its primary block and host-issued reference handles. The host also owns cycle prevention for nested editors and all credential, filesystem, clipboard, network, and native-handle policy.

Replace the fixed `plugin-demo` path and global debug window with package lookup and ordinary `EditorRegistry` registrations. Registry entries become data from validated plugin manifests plus host callbacks for opening and creating `PluginEditor` instances. Unsupported, missing, incompatible, and crashed plugins use distinct user-facing states so installation errors are not confused with block permissions or loading.

### Rendering and platform support

Require external GPU surfaces on every target. Windows uses DXGI shared textures, Linux uses DMA-BUF, macOS uses IOSurface, Android uses `AHardwareBuffer`, and web uses `WebExternalImage` backed by the plugin's off-screen canvas. The handshake rejects a plugin when the host and guest cannot negotiate the platform's required surface mechanism. There is no software or pixel-buffer presentation path.

Named regions are independent viewports with their own size, scale factor, input focus, and frame generation. This preserves host-owned panel layout and allows previews and creation dialogs to exist without opening a full tab. Regions that do not need continuous animation render on state, input, theme, access, or size changes. The host sends theme and font-scale data so first-party plugins remain visually consistent, while the protocol does not expose internal `egui` objects.

Linux DMA-BUF, macOS IOSurface, and Android hardware-buffer import/export are not currently completed by the demo, so they are explicit platform-enablement milestones. Surface loss recreates the optimized surface and its region without losing the editor instance; repeated recreation failure puts that region into an error state rather than changing presentation mechanisms.

### SDK and editor layout

Split the current demo into three responsibilities:

- `block-plugin-api`: serialization-only protocol types, validation, host and guest session state machines, and attachment descriptors, with no UI framework dependency;
- a new `block-editor-plugin` guest SDK: instance routing, revision handling, host requests, `egui` input adaptation, region rendering, and target entry points;
- `plugin-demo`: a minimal reference package that declares one test block editor and exercises every stable capability needed by conformance tests.

First-party editor packages depend on the guest SDK and the block model types they edit, but not on `block-app` or `BlockClient`. To make that possible without pulling networking into every plugin, move each block's state and operation definitions from `block-client::blocks` into model-only crates or a model-only `block-types` crate. `block-client` re-exports those types and supplies typed handles; plugins use the same codecs to read snapshots and construct operations. Editor-only reusable cores such as `logicgame`, canvas geometry, database layouts, and renderers move with or below their plugin packages rather than remaining reachable through `block-app`.

Split the current text editor into two reusable layers before migrating it. `text-editor-core` remains the UI-independent editing engine for diffing, syntax highlighting, and cursor data. A new `text-editor-view` package owns the editor/view behavior currently in `block-app`, including `egui` interaction, layout, font handling, embeds, selection, cursor presentation, and the adapter that drives `text-editor-core`. The text plugin depends on `text-editor-view` and connects its changes and presence data to the guest SDK; neither reusable text package depends on `block-app` or plugin transport.

The guest SDK should present an API close to today's editor authoring model: a typed state view, an operation sender, local UI state, reference and presence subscriptions, named region callbacks, and declarative capabilities. It must not imitate `BlockHandle` in a way that suggests synchronous reads or successful writes; state revisions and rejected operations are explicit. Provide a package template, manifest validation command, build command, and a revised editor guide based on `plugin-demo`.

### Migration and completion criteria

Migrate editors in increasing order of dependency complexity. Begin with read-only and simple single-block editors, continue with ordinary editable blocks, then move media and configurable creation, nested/reference-heavy editors, GPU editors, and finally text, logic grid, database views, and dynamic-artifact producers. During migration, `EditorRegistry` may contain both native and plugin registrations, but a block type has exactly one selected implementation.

An editor is considered migrated only when its creation flow, direct tab, embedded mode, preview, access changes, undo/redo, presence, references, and failure recovery match the capabilities it had in process. Deterministic non-GUI editor logic keeps ordinary Rust tests in its model or core crate. Protocol, SDK, lifecycle, and host adapter behavior receive automated tests; visual and platform-surface behavior receives a documented manual test matrix.

The migration is complete when `block-app/src/editors` contains only the plugin adapter, host-owned editor layout and common chrome, unsupported/error UI, registry/catalog integration, and truly app-wide helpers. There are no first-party block-type editor implementations linked into `block-app`; deleting or disabling a plugin package makes that editor unavailable without recompiling the host; and the same package artifacts and protocol are used by the reference demo and production editors.

## Suggested units of work

The first milestone is deliberately a narrow vertical slice: a real synchronized counter block rendered and edited by a plugin in an ordinary tab. It uses the existing WebExternalImage and Windows DXGI work, supports one plugin instance and one main region, and omits creation UI, previews, history controls, presence, references, and third-party discovery. That proves the model boundary and end-to-end texture path before the general framework is built around it.

1. **Add the counter block model.** Create a model-only counter state and increment/decrement operations, re-export it from `block-client`, and add serialization, operation, and history tests.
2. **Define the minimum editor manifest.** Add bounded manifest types for plugin identity, one block type, display metadata, entry points, and a required surface mechanism; give `plugin-demo` a valid manifest.
3. **Define the minimum editor protocol.** Add instance IDs plus open, snapshot, operation, resize, input, close, and error messages for one main region, with codec-limit and ordering tests.
4. **Add the counter's host block adapter.** Encode counter snapshots, decode counter operations, apply them through its typed `BlockHandle`, and reject invalid payloads or edits without access.
5. **Extract the minimum host runtime.** Move the existing transport, handshake, process/WASM lifecycle, input queue, and surface presentation out of the debug window into a reusable host module.
6. **Extract the minimum guest SDK.** Move handshake, single-instance routing, revisioned counter state, operation submission, `egui` input, and frame production from `plugin-demo` into `block-editor-plugin`.
7. **Add the counter plugin editor.** Replace the local demo controls with a counter view that reads host snapshots and sends increment/decrement operations through the guest SDK.
8. **Open the counter through `EditorRegistry`.** Add a minimal `PluginEditor`, register the counter manifest, open it as a normal tab, and remove the counter from the debug-only plugin window. This commit is the MVP.
9. **Support multiple instances per runtime.** Route instance-scoped messages, start one runtime per package, close it after its last instance, and isolate stale or duplicate instance IDs.
10. **Add robust revision handling.** Acknowledge or reject operations with authoritative revisions, replace state after concurrent changes, bound pending operations, and test resynchronization.
11. **Add live access and history.** Send access-ceiling and history-availability changes, route undo, redo, and history grouping, and reject mutations immediately after access is reduced.
12. **Add package failure recovery.** Distinguish missing, incompatible, crashed, timed-out, and surface-failed packages; restart first-party runtimes and reopen their instances from host state.
13. **Generalize Windows texture presentation.** Move DXGI sharing fully behind the region presenter, support surface recreation, and put a failed region into an error state without another presentation path.
14. **Generalize web texture presentation.** Replace the fixed demo canvas with per-package, per-region external images and deterministic GPU and DOM resource teardown.
15. **Implement Linux texture presentation.** Add DMA-BUF export, attachment transfer, import, synchronization, surface recreation, and lifecycle tests.
16. **Implement macOS texture presentation.** Add IOSurface export/import, synchronization, signing-safe attachment handling, surface recreation, and lifecycle tests.
17. **Implement Android texture presentation.** Add the Android service transport plus `AHardwareBuffer` transfer, import, synchronization, surface recreation, and lifecycle tests.
18. **Make optimized surfaces mandatory.** Require the platform surface capability during negotiation, remove demo-only surface assumptions, and test that a handshake with no common mechanism fails explicitly.
19. **Add named editor regions.** Generalize instances to main, top bar, left sidebar, right sidebar, preview, creation-options, and artifact-settings regions with independent size, focus, input, and frame generations.
20. **Complete the `PluginEditor` adapter.** Map every current `BlockEditor` region, intrinsic-size query, pan/zoom capability, viewport command, and navigation result onto plugin messages.
21. **Add creation flows.** Extend manifests and the protocol for immediate and configured creation, let the host create the authoritative block, and make the counter available through the normal block picker.
22. **Add scoped reference access.** Implement watch, unwatch, snapshot, and change messages using host-issued handles, and test that plugins cannot address arbitrary block UUIDs.
23. **Add nested editors and previews.** Let a plugin request host-rendered referenced previews or live regions while the host enforces access ceilings and cycle prevention.
24. **Add presence.** Implement bounded presence subscriptions and updates keyed by well-known presence UUIDs, with access enforcement and cleanup when instances close.
25. **Add child and artifact actions.** Route child creation/replacement/deletion plus dynamic-artifact settings and regeneration results through host-owned validation.
26. **Create the full guest authoring API.** Add typed state views, operation senders, reference and presence subscriptions, declarative regions, theme/font-scale data, and a headless conformance harness.
27. **Generate the first-party catalog.** Validate manifests during workspace builds, generate deterministic registrations, reject duplicate block types, and load catalog registrations beside native editors.
28. **Add external discovery and packaging.** Discover desktop packages, resolve explicit overrides, and teach native, web, and Android builds to stage catalog entry points, manifests, and assets.
29. **Turn `plugin-demo` into the conformance package.** Keep the counter as its reference editor and exercise creation, preview, history, presence, references, access changes, multiple instances, and intentional failure modes.
30. **Extract the first migration models.** Move compiled logic, calendar, pixel art, presentation, and workspace-index state and operations into model-only crates or `block-types`, preserving re-exports and tests.
31. **Migrate compiled logic and calendar.** Package their direct and preview regions, register their manifests, and remove their in-process editor modules.
32. **Migrate pixel art and workspace index.** Package painting, image generation, listings, and child navigation, then remove their native registrations.
33. **Migrate presentation.** Package slide creation, reference watching, embedded previews, presenter playback, and child actions, then remove its native editor.
34. **Add scoped import and media services.** Define host requests for file selection, clipboard images, media decode/playback, and native webviews with per-instance grants and bounded results.
35. **Migrate image, audio, and browser tab.** Use the scoped services for imports, playback, and platform webviews while retaining manifest-declared target availability.
36. **Migrate video.** Package its timeline, player, and effects using host media services while preserving frame-accurate operations.
37. **Add scoped map and GPU services.** Define host-mediated tile fetching/cache access and editor GPU-resource initialization without granting credentials or arbitrary filesystem/network access.
38. **Migrate map and pixel ray tracer.** Package their rendering cores and UI using the scoped services, including previews and intrinsic sizing.
39. **Migrate scene 3D.** Package its renderer, shaders, camera, and scene UI, then remove its block-app render-resource installation.
40. **Migrate settings, UI settings, and hotbar.** Package fallback and per-client settings resolution plus the shared nested hotbar, then remove their native editors.
41. **Migrate logic game and version control.** Package challenge, quiz, version-control data, and worktree UI while keeping privileged repository work in scoped host APIs.
42. **Migrate infinite canvas.** Package its core, interaction, painting, inspector, geometry, nested live editors, previews, and clipboard integration.
43. **Migrate GUI builder.** Package its surface, inspector, nested block picker, and dynamic Rust artifact settings and regeneration.
44. **Extract database editor cores.** Move schema, spreadsheet, kanban, and scatter logic below a plugin package and expose the shared model types without depending on block-app.
45. **Migrate database editors.** Package database, database schema, and database view together, preserving references, configured layouts, sorting, and embedded behavior.
46. **Create `text-editor-view`.** Move `egui` interaction, layout, fonts, embeds, selection, and cursor presentation out of block-app into a package layered on `text-editor-core`, with neither package depending on plugin transport.
47. **Migrate text.** Build the text plugin around `text-editor-view`, connecting revisioned text state, operations, cursor presence, embeds, and intrinsic sizing to the guest SDK.
48. **Extract logic-grid editor cores.** Move canvas geometry, simulation, challenges, hotbar logic, renderer, shaders, and compiled-artifact generation below a plugin package.
49. **Migrate logic grid.** Package direct editing, GPU rendering, presence, challenge playback, nested components, and compiled-logic artifacts, then remove its native editor.
50. **Remove the in-process editor path.** Delete remaining block-type registrations and modules, reduce block-app to host chrome and the plugin adapter, and remove debug demo entry points and fixed paths.
51. **Ship the authoring workflow.** Replace the editor guides, add package scaffold and manifest/build commands, document compatibility and signing, and add CI plus the desktop/web/Android manual matrix.
