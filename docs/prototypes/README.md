# Prototypes

This directory holds review-only static prototypes.

Rules:

- prototypes are not runtime truth
- prototypes are not protocol truth
- prototypes must say clearly when data is mocked
- prototypes must not silently drift into production app assets

Current entries:

- `mobile-ui-tree/index.html`
  - review-only static UI tree for the phone shell entrance model: icon-only corner entrances, persisted session history, timer dashboard, and current-session dashboard
- `mobile-ui-tree/config.html`
  - review-only static config entrance audit: provider registry, model groups, daemon/runtime, skills, memory, MCP, Android shell permissions/update, logs, and about
- `webui-console/index.html`
  - workspace page for high-frequency operational actions only
- `webui-console/settings.html`
  - separated low-frequency settings page
