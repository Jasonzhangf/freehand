# WebUI Information Retrieval Design Validation

- status: validated against `frontend-ui-reference`, `impeccable`, and `ux-audit`.
- notifications: rank by importance first, then time; the notification surface is for attention triage, not physical Agent navigation.
- topology: owns physical classification and Agent location. Agent identity, role, node, and parent/worker relationships remain protocol/config/task projections.
- canvas: owns current, recent, and historical session continuity. Each runtime round remains a separate semantic cycle card in chronological order.
- search plugin: must expose typed input, result projection, persistence/index ownership, and cache lifecycle. The UI cannot search browser-local transcript state.
- memory plugin: must expose typed attach/search/load/save/export operations. Per-session summaries are inputs to the plugin, but durable memory is independent of session deletion.
- current delivery boundary: this issue adds the typed tool-result memory write operation and configurable server-owned storage; search and automatic session summarization remain separate follow-up feature slices.

## Reference Decisions

- Beautiful UI AI patterns: expandable tool/status records and progressive detail for live agent work.
- shadcn-style command/input primitives: semantic buttons, explicit focus, and command surfaces with keyboard access.
- UX audit: task-first hierarchy, visible loading/error states, 44px touch targets, and no color-only status semantics.
