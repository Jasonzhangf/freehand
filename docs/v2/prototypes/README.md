# v2 UI Prototypes

These files are review-only static prototypes. They simulate typed Cordis
plugin projections through a local `UiAdaptor`; they are not runtime truth and
must not be used as a production UI entrypoint.

## Console

Open:

```text
docs/v2/prototypes/v2-ui-plugin-console/index.html
```

The prototype covers the v2 operating surfaces:

- `Location / Topology`
- `Run`
- `Attention / Notifications`
- `Sessions / Canvas`
- `More` with grouped `Search`, `Memory`, `Settings`, `Timer`, `Tools` and
  `New session` entries on mobile

Desktop keeps the v1 shell entrypoints visible in the header. Mobile exposes
the same capabilities through one `More` entry with three groups:

- 操作: New session, Timer dashboard
- 信息: Search, Memory
- 系统: Settings, Tools registry

The underlying surfaces remain complete:

- Settings: Models, Agent runtime, Connectivity, Observability, Appearance,
  About
- Timer dashboard: relative, absolute, interval, daily, weekly and cron
  scheduling, source session, reason, wake prompt, max runs and history
- Tools registry: schema, exposure, permission, example, invoke and detach
- Session search
- New session

All controls use simulated projection state and provide visible interaction
feedback. The prototype does not persist browser state, call the v1 server,
write Session Log truth, execute tools, or create real timers.

The prototype mirrors the frozen UI plugin slots:

- `ui.shell`: page frame and modal host
- `ui.navigation`: one-row mobile route selection
- `ui.run`: current run and composer
- `ui.sessions`: session rail and canvas entry
- `ui.attention`: notification rail
- `ui.location`: topology rail
- `ui.more`: grouped mobile secondary capabilities
- `ui.detail`: modal detail surfaces

These slots are represented in one static page only for review convenience.
The production implementation must keep their contracts independent so one UI
plugin can be replaced without changing domain plugin truth.
