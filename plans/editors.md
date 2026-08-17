# Plugin-based block editors

## Proposal

### Goals

- Make the plugin boundary the only supported way to implement a block editor. `block-app` should be an editor host, while every first-party editor is built, packaged, discovered, and run through the same public API available to third-party editors.
- Keep synchronized block models in the trusted host. A plugin edits a host-owned block through serialized snapshots and operations; it does not connect to the server, own a `BlockClient`, implement permissions, or duplicate undo/redo, presence, relationship, and dynamic-artifact rules.
- Preserve the behavior of the current `BlockEditor` surface: creation, tabs, embedded live editors, passive previews, top and side regions, intrinsic sizing, pan and zoom, access control, child blocks, navigation actions, presence, and dynamic artifacts.
- Isolate editor failures. A malformed message, panic, timeout, or crash should close only the affected editor instance and show a recoverable error in its place.
- Support desktop, web, and Android before the in-process editor path is removed. GPU surface sharing may remain an optimization, but every supported target needs a correct fallback presentation path.
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

Keep the current external-surface mechanisms as fast paths, but add a portable software-frame capability. Native platforms should transfer bounded RGBA buffers through shared memory or platform attachments rather than embedding large frames in protocol messages. Web may continue copying an off-screen canvas into the host texture. Android needs a service/process transport and either `AHardwareBuffer` presentation or the software fallback.

Named regions are independent viewports with their own size, scale factor, input focus, and frame generation. This preserves host-owned panel layout and allows previews and creation dialogs to exist without opening a full tab. Regions that do not need continuous animation render on state, input, theme, access, or size changes. The host sends theme and font-scale data so first-party plugins remain visually consistent, while the protocol does not expose internal `egui` objects.

Linux DMA-BUF and macOS IOSurface import/export are not currently completed by the demo and must not gate correctness. Implement the software path first on every target, then retain Windows DXGI, WebExternalImage, DMA-BUF, IOSurface, and Android hardware buffers as negotiated accelerations. Surface loss must fall back or recreate the region without losing the editor instance.

### SDK and editor layout

Split the current demo into three responsibilities:

- `block-plugin-api`: serialization-only protocol types, validation, host and guest session state machines, and attachment descriptors, with no UI framework dependency;
- a new `block-editor-plugin` guest SDK: instance routing, revision handling, host requests, `egui` input adaptation, region rendering, and target entry points;
- `plugin-demo`: a minimal reference package that declares one test block editor and exercises every stable capability needed by conformance tests.

First-party editor packages depend on the guest SDK and the block model types they edit, but not on `block-app` or `BlockClient`. To make that possible without pulling networking into every plugin, move each block's state and operation definitions from `block-client::blocks` into model-only crates or a model-only `block-types` crate. `block-client` re-exports those types and supplies typed handles; plugins use the same codecs to read snapshots and construct operations. Editor-only reusable cores such as `text-editor-core`, `logicgame`, canvas geometry, database layouts, and renderers move with or below their plugin packages rather than remaining reachable through `block-app`.

The guest SDK should present an API close to today's editor authoring model: a typed state view, an operation sender, local UI state, reference and presence subscriptions, named region callbacks, and declarative capabilities. It must not imitate `BlockHandle` in a way that suggests synchronous reads or successful writes; state revisions and rejected operations are explicit. Provide a package template, manifest validation command, build command, and a revised editor guide based on `plugin-demo`.

### Migration and completion criteria

Migrate editors in increasing order of dependency complexity. Begin with read-only and simple single-block editors, continue with ordinary editable blocks, then move media and configurable creation, nested/reference-heavy editors, GPU editors, and finally text, logic grid, database views, and dynamic-artifact producers. During migration, `EditorRegistry` may contain both native and plugin registrations, but a block type has exactly one selected implementation.

An editor is considered migrated only when its creation flow, direct tab, embedded mode, preview, access changes, undo/redo, presence, references, and failure recovery match the capabilities it had in process. Deterministic non-GUI editor logic keeps ordinary Rust tests in its model or core crate. Protocol, SDK, lifecycle, and host adapter behavior receive automated tests; visual and platform-surface behavior receives a documented manual test matrix.

The migration is complete when `block-app/src/editors` contains only the plugin adapter, host-owned editor layout and common chrome, unsupported/error UI, registry/catalog integration, and truly app-wide helpers. There are no first-party block-type editor implementations linked into `block-app`; deleting or disabling a plugin package makes that editor unavailable without recompiling the host; and the same package artifacts and protocol are used by the reference demo and production editors.

## Suggested units of work

1. **Define the package manifest.** Add manifest types, bounded decoding, semantic validation, and isolated tests to `block-plugin-api`; give `plugin-demo` a checked manifest without changing how it runs.
2. **Define editor instances and regions.** Add instance IDs, roles, capabilities, named-region descriptors, geometry types, and their codec-limit tests to the protocol.
3. **Implement instance lifecycle messages.** Add create, ready, update, close, and failure messages to both session state machines, with ordering and cleanup tests.
4. **Implement revisioned block messages.** Add snapshot, properties, operation request, accepted operation, rejected operation, and history-group messages, including stale-revision and payload-limit tests.
5. **Add type-erased block adapters.** Let a host registration encode a typed block snapshot and decode a typed operation before applying it through `BlockHandle`; cover unknown types and invalid payloads with non-GUI tests.
6. **Add history and access messages.** Route undo, redo, history availability, and live access-ceiling changes, and reject mutations from non-editable instances in host-session tests.
7. **Add scoped reference messages.** Implement watch, unwatch, snapshot, and change messages using host-issued reference handles; test that a plugin cannot address an arbitrary block UUID.
8. **Add presence messages.** Implement bounded presence subscriptions and updates keyed by well-known presence UUIDs, with host authorization and cleanup when an instance closes.
9. **Add editor action messages.** Cover navigation, child actions, intrinsic-size changes, viewport commands, creation results, and dynamic-artifact results without exposing `egui` or app trait types.
10. **Extract the transport runtime.** Move framing, handshake, queues, timeouts, and shutdown out of `debug/plugin_demo` into a reusable block-app host module while keeping the demo window working.
11. **Add package runtime management.** Start one runtime per package, route multiple instance IDs over it, stop it after the last instance closes, and surface crashes independently from other packages.
12. **Generalize input and region scheduling.** Give every named region independent focus, input, scale, resize, frame generation, coalescing, and repaint state.
13. **Add the `PluginEditor` migration adapter.** Implement the current `BlockEditor` methods by opening plugin regions and translating actions, with loading, missing, incompatible, failed, and restart states.
14. **Add native software frames.** Define attachment-backed RGBA buffers, implement the desktop producer and presenter, and test size limits, ownership, frame replacement, and release.
15. **Add web software frames.** Generalize the off-screen canvas adapter from the fixed demo canvas to per-package, per-region canvases with deterministic teardown.
16. **Add the Android correctness path.** Implement the Android service transport and software-frame presentation so plugins work before hardware-buffer acceleration is available.
17. **Integrate Windows accelerated frames.** Move the existing DXGI producer and presenter behind general surface negotiation and make loss fall back to software frames.
18. **Implement Linux accelerated frames.** Add DMA-BUF export, attachment transport, import, synchronization, recovery, and the existing lifecycle tests for the real presenter.
19. **Implement macOS accelerated frames.** Add IOSurface export/import, synchronization, signing-safe attachment handling, recovery, and the existing lifecycle tests for the real presenter.
20. **Implement Android accelerated frames.** Add `AHardwareBuffer` negotiation, transfer, presentation, synchronization, and fallback without changing the editor contract.
21. **Create the guest runtime SDK.** Extract target entry points, handshake, instance routing, revision tracking, and host-request clients from `plugin-demo` into `block-editor-plugin`.
22. **Create the guest `egui` SDK.** Extract input conversion, theme and font-scale handling, named-region execution, frame production, and a headless region harness.
23. **Generate the first-party catalog.** Validate manifests during workspace builds, generate deterministic registrations, reject duplicate block types, and load those registrations beside native editors.
24. **Add external discovery and packaging.** Discover additional desktop packages, resolve explicit overrides, and teach native, web, and Android build scripts to stage the catalog entry points and assets.
25. **Convert `plugin-demo` into the conformance editor.** Use the guest SDK and a real simple block to exercise creation, editing, preview, history, presence, references, access changes, and intentional failure modes.
26. **Extract the simple model types.** Move compiled logic, calendar, pixel art, presentation, and workspace-index state and operations into model-only crates or `block-types`, preserving `block-client` re-exports and serialization tests.
27. **Migrate compiled logic and calendar.** Create their plugin packages, register them through manifests, cover their existing direct and preview capabilities, and remove the in-process editor modules.
28. **Migrate pixel art and workspace index.** Move their editor cores and UI into plugin packages, including pixel-art image generation and workspace-index child navigation, then remove the native registrations.
29. **Migrate presentation.** Package slide creation, reference watching, embedded previews, presenter playback, and child actions, then remove its in-process implementation.
30. **Add scoped import and media services.** Define host requests for file selection, clipboard image import, media decode/playback, and native webviews, with per-instance grants and bounded results.
31. **Migrate image, audio, and browser tab.** Use the scoped services for configurable imports, playback, and platform webviews, retaining unsupported-target behavior through manifest capabilities.
32. **Migrate video.** Move its timeline, player, and effects into a plugin package using host media services and preserve frame-accurate block operations.
33. **Add scoped map and GPU services.** Define host-mediated tile fetching/cache access and negotiated editor-owned GPU resource initialization without granting credentials or arbitrary filesystem/network access.
34. **Migrate map and pixel ray tracer.** Move their reusable rendering cores and UI into packages using the new scoped services, including preview and intrinsic-size behavior.
35. **Migrate scene 3D.** Move its renderer, shader assets, camera, and scene UI into a package and remove the scene render-resource installation from block-app.
36. **Migrate settings, UI settings, and hotbar.** Package fallback and per-client settings resolution plus the shared nested hotbar, then remove their native registrations.
37. **Migrate logic game and version control.** Package challenge/quiz UI and version-control data/worktree UI while keeping simulation and repository operations in scoped host or model APIs.
38. **Migrate infinite canvas.** Move its core, interaction, painting, inspector, and geometry into a package and validate nested live editors, previews, clipboard import, access ceilings, and cycle prevention.
39. **Migrate GUI builder.** Move its surface and inspector into a package, including nested block picking and dynamic Rust artifact settings and regeneration.
40. **Extract database editor cores.** Move schema, spreadsheet, kanban, and scatter logic below plugin packages and expose shared model types without depending on block-app.
41. **Migrate database editors.** Package database, database schema, and database view together, preserving reference subscriptions, configured layouts, sorting, and embedded behavior.
42. **Migrate text.** Package the text UI around `text-editor-core`, including concurrent snapshot replacement, syntax highlighting, embedded images, cursor presence, and intrinsic sizing.
43. **Extract logic-grid editor cores.** Move canvas geometry, simulation, challenges, hotbar logic, renderer, shaders, and compiled-artifact generation below a plugin package.
44. **Migrate logic grid.** Package direct editing, GPU rendering, presence, challenge playback, nested components, and dynamic compiled-logic artifacts, then remove its native registration.
45. **Remove the in-process editor path.** Delete the remaining block-type modules and registrations, reduce `BlockEditor` to host chrome and the plugin adapter, and remove the debug demo window and fixed plugin paths.
46. **Ship the editor authoring workflow.** Replace the editor guides, add package scaffold and manifest/build commands, document compatibility and signing, and add CI plus a desktop/web/Android manual test matrix for every first-party package.
