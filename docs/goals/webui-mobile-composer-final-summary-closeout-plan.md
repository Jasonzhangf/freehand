# WebUI Mobile Composer And Final Summary Closeout Plan

## Goal And Acceptance

Close the current WebUI/mobile UI defects:

- Phone focused composer must not show meaningless blocking content above the input.
- Final/Summary output must reflect the actual response format: plain single-line prose renders as one readable paragraph, while explicit source structure such as newlines, bullets, numbering, or line-start labels renders as structured blocks.
- The behavior must be verified on the real WebUI surface, not only by static tests.

Acceptance:

- On phone/tall-phone layout, focusing the composer may enlarge the input and expose cancel, but must not render the full attachment/CWD/model/status strip into the main conversation flow.
- Empty attachment/status text such as `no draft attachments` and completed command status must not reserve vertical space above the input.
- Final rows use a dedicated summary renderer that preserves the source response format while keeping evidence/learned/completion reason hidden unless debug details are enabled.
- Served daemon assets match workspace assets.
- Browser/mobile evidence proves the visual behavior on the live page.

## Scope

In scope:

- `apps/freehand-server/assets/webui.css`
- `apps/freehand-server/assets/webui.js`
- `apps/freehand-server/src/lib.rs` asset smoke locks
- `docs/function-maps/app.webui-smoke.md`
- `docs/testing/app.webui-smoke.md`
- Browser/WebUI online verification evidence under `artifacts/webui-online/`

Out of scope:

- ADP protocol changes.
- Reasoning/session truth changes.
- Attachment backend storage changes.
- Android native shell redesign unless release/phone validation requires packaging proof.
- Fake controls, fallback paths, or UI-only truth mutation.

## Design Principles

- UI consumes protocol/session truth only; display parsing cannot mutate truth.
- Mobile primary surface is the conversation, not status/debug/config chrome.
- Low-frequency controls stay behind explicit controls/drawers and must not occupy focused input space by default.
- Final summary formatting is display-only and preserves source text semantics.
- Final summary rendering must not infer structure from business words, punctuation density, or domain-specific terms.
- Debug fields stay hidden by default.

## Technical Plan

1. Update test design and function map first to lock the intended behavior.
2. In CSS, keep phone/tall/tablet portrait focused composer compact:
   - shrink focused composer max height;
   - keep conversation/message bottom padding aligned to compact composer height;
   - keep `composer-control-strip`, `attachment-tray`, and `command-status` hidden on focused mobile layouts.
3. In JS, route `row.kind === "final"` through a dedicated final summary renderer:
   - normalize text;
   - split only by existing line breaks;
   - parse line-start `label: value` / `label：value`, parenthesized labels, bullets, and numbered labels when they are already present in the response;
   - do not split a plain one-line summary by sentence punctuation or domain words;
   - render source-format blocks with dedicated classes.
4. Add asset smoke assertions for:
   - final summary renderer functions/classes;
   - compact mobile focused composer CSS rules.
5. Verify locally and online.

## Risks And Guardrails

- Risk: hiding composer controls may hide attachment affordances on mobile.
  - Guardrail: do not remove DOM or functionality; only prevent automatic focused-state expansion from occupying the conversation flow.
- Risk: summary splitting could invent a structure the model did not return.
  - Guardrail: only project source-format markers; do not split by punctuation, keywords, or inferred business semantics, and do not modify protocol payload/session truth.
- Risk: static checks miss browser runtime errors.
  - Guardrail: run real browser/WebUI verification against a restarted daemon with served asset hash match.
- Risk: stale daemon assets make screenshots meaningless.
  - Guardrail: compare workspace and served `webui.js` / `webui.css` hashes before browser evidence.

## Verification Matrix

Local:

- `node --check apps/freehand-server/assets/webui.js`
- `cargo test -p freehand-server -- --nocapture`
- `cargo fmt --check`
- `git diff --check`
- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`

Online:

- `scripts/install-launchd.sh restartS`
- `curl -4fsS http://127.0.0.1:4042/health`
- `~/.local/bin/freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp`
- Compare served and workspace SHA-256 for `webui.js` and `webui.css`
- Drive real WebUI in browser at phone/tall viewport:
  - focus composer;
  - assert hidden focused mobile controls/status/tray;
  - assert compact composer height;
  - assert a plain one-line Final summary renders as one `.final-summary-item`;
  - assert a structured Final summary with source line breaks renders matching multiple `.final-summary-item` blocks;
  - save screenshot and DOM summary under `artifacts/webui-online/`.

Release/phone, only if this task is promoted to release proof:

- `scripts/install-global.sh`
- `FREEHAND_DAEMON_BIND=100.66.1.82:4041 scripts/install-launchd.sh restart`
- release health, ADP smoke, served hash match
- Android true-device WebView verification against `100.104.163.65:5555`

## Implementation Steps

1. Confirm `app.webui-smoke` owner and read function/test maps.
2. Update docs/test-design constraints.
3. Patch WebUI CSS and JS.
4. Add/adjust server asset smoke locks.
5. Run local verification matrix.
6. Restart S profile and verify health/ADP/hash.
7. Run real browser mobile validation and save evidence.
8. Update `note.md` and, if fully verified, `MEMORY.md` / local skill with reusable lesson.
9. Report only verified facts, remaining gaps, and evidence paths.

## Definition Of Done

- The focused phone composer no longer shows the meaningless blocking strip above/below the input.
- Final/Summary is readable and source-format preserving: plain source stays plain, structured source stays structured.
- Local gates pass.
- Live WebUI evidence proves the changed behavior with current served assets.
- No unrelated dirty files are reverted or overwritten.
