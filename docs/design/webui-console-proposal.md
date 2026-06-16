# WebUI Console Proposal

## Status

Proposal for review only.

- not yet approved as durable runtime truth
- not yet wired into `apps/freehand-server`
- current prototype is static and offline

Prototype entries:

- `docs/prototypes/webui-console/index.html`
- `docs/prototypes/webui-console/settings.html`

## Redesign Intent

The previous prototype failed for four reasons raised in review:

1. it tried to place too many responsibilities on one page
2. low-frequency configuration and maintenance actions were mixed into the main workspace
3. mobile behavior was treated as simple responsive compression instead of a future first-class phone UI
4. page layering did not match the calmer, more structured application shell style seen in OpenCode

This redesign fixes those four points directly.

## Reference Direction

The structural reference is closer to OpenCode than to a dashboard wall:

- narrow global navigation rail
- focused secondary list pane
- one dominant conversation canvas
- one contextual inspector pane
- settings separated from the main work surface

What we borrow is structure, not visual cloning.

What we explicitly learned from inspecting OpenCode:

- the main surface is still a conversation/chat workspace
- the input composer is a persistent primary control
- execution steps appear as semantic cards inside the conversation flow
- tool/shell/process output is summarized for reading instead of exposing raw transport payload by default

Latest visual correction from review:

- the conversation is the primary focus
- statistics must be reduced to low-emphasis inline status, not a large card group
- color should be calm and readable, not loud dashboard blue
- common actions must stay near the composer; rare actions stay out of the main path
- side panes must visually retreat so the conversation and composer become the dominant center
- the conversation stream should be width-bounded and centered, not spread across a giant empty canvas

Latest region-autonomy correction from review:

- top bar is its own region and must own master/slave status
- collapsed top bar shows slave agent name and state, such as busy or idle
- top bar can expand to show selectable slave agent cards
- conversation region must have one fixed message-block format
- assistant/model messages render on the left; user messages render on the right
- text, tool, success, running, and failure states each need fixed colors
- each message block has a simple state view plus a details disclosure
- the slave region should behave like a collapsed control strip first and an expanded card set second
- the selected-block region should stay narrow and secondary instead of competing with the main stream
- the composer must be physically larger than a casual chat input so it can hold multi-step operator instructions
- message cards should use whole-card colored borders and colored title rows, not only faint side markers
- the first prototype should support both white and black themes with one shared semantic color system

## Hard Constraints From Confirmed Repo Truth

| area | locked truth |
| --- | --- |
| owner boundary | WebUI is a protocol consumer, not a reason/provider/node/config owner |
| mutation authority | UI can submit commands only; UI cannot directly write reason/debug/session truth |
| transport split | query is snapshot-only, subscribe is incremental-only, command ingress is separate |
| first transport | HTTP query + SSE subscribe + POST command ingress |
| shared truth | CLI and WebUI must share `freehand-ui-protocol` semantics |
| WebUI-only behavior | slave turn appears as a separate WebUI substream |
| debug boundary | UI may show debug projection, not raw provider ledgers as truth |
| runtime host | `apps/freehand-daemon` hosts runtime; `apps/freehand-server` stays protocol-only |

## Page Taxonomy

The WebUI is no longer modeled as one page.

### 1. Workspace Page

This is the default landing page and the only page that keeps high-frequency operational work.

Allowed responsibilities:

- current active turn
- session watch list
- command composer
- turn stream
- focused inspector for trace/debug or slave substream summary

Not allowed here:

- provider configuration editing
- agent pairing setup
- runtime directory settings
- replay/debug retention policy settings

### 2. Settings Page

This page owns low-frequency configuration and maintenance operations.

Settings groups in the proposal:

- Providers
- Agents
- Pairing and network policy
- Debug and replay retention
- UI preferences
- Runtime directories and home

This is where classified configuration belongs. It should not compete with live turn reading.

### 3. Future Pages After Approval

These are intentionally separated from the first workspace:

- Nodes
- Replay
- Debug

They are listed in navigation, but the current prototype only renders workspace and settings.

## Workspace Information Architecture

### Global Rail

Purpose:

- global section switch
- current agent identity
- quick movement between Workspace and Settings

Future phone mapping:

- desktop left rail becomes mobile bottom navigation

### Session Pane

Purpose:

- high-frequency session switching
- latest-turn watch context
- recent sessions list

Current backing status:

| item | backing status |
| --- | --- |
| active turn/session summary | partially backed by current latest-turn query |
| full session list | mock-only until session/index query exists |

### Main Work Canvas

Purpose:

- one dominant conversation surface for the active turn
- current command composer as a persistent primary control

Main canvas includes only:

- turn header
- user message
- assistant message blocks
- tool call/result semantic cards
- terminal projection
- command composer
- low-emphasis inline status such as streaming/tool/cache/schema

It does not carry settings or maintenance controls.

Important correction:

- this is chat-first for the active turn, not dashboard-first
- semantic events must be projected into readable conversation units
- raw JSON or raw provider payload should not be the default reading mode
- token/cost/debug numbers must not compete with the active conversation
- top/slave status, conversation flow, composer, and selected-block detail are autonomous regions

### Inspector Pane

Purpose:

- current context only
- not all metadata at once

The inspector uses tabs:

- Trace
- Slave
- Usage

Default desktop state:

- one active tab visible
- no giant all-in-one right column

Future phone mapping:

- inspector becomes an in-page segmented section under the main stream

## Settings Information Architecture

### Provider Settings

Contains:

- provider id
- protocol
- auth source type
- default model
- enable/disable state

### Agent Settings

Contains:

- agent name
- mode
- bound provider
- node id
- paired agent

### Pairing and Network Policy

Contains:

- allowed pair IP policy
- pair token env name
- listening interface
- reconnect / relisten notes

### Debug and Replay

Contains:

- debug sink switches
- replay retention
- ledger visibility policy
- file output policy

### UI Preferences

Contains:

- default landing page
- default inspector tab
- compact / comfortable density

### Runtime Directories

Contains:

- `~/.freehand`
- `state`
- `ledgers`
- `cache`
- `replays`
- `logs`

## Mobile-First Rules

The phone version is not "desktop squeezed smaller". It has its own navigation model.

### Navigation

- left rail becomes bottom navigation
- session pane becomes a top sheet or dedicated Sessions view
- settings stay on a separate page, not hidden in the workspace canvas

### Workspace on Phone

Order:

1. compact turn header
2. active turn conversation
3. command composer
4. inspector tabs
5. session context entry

Why:

- active dialogue and input remain near the thumb zone
- low-frequency browsing is pushed outward
- the dominant task remains "read current turn conversation and send next command"

### Settings on Phone

- settings categories become stacked sections
- each category stays collapsible
- editing flows should drill into focused detail screens later if needed

## Binding Matrix

| screen region | intended source | current repo status |
| --- | --- | --- |
| workspace header | latest-turn query | partial |
| session pane | future session/index query | mock only |
| turn stream | latest-turn query + latest-turn SSE | backed by current transport shape |
| command composer | POST `/ui/command` | backed by current transport shape |
| trace inspector | debug query + debug SSE | backed by current transport shape |
| slave inspector tab | WebUI turn projection / slave substream | semantically backed; transport expansion still needed |
| settings page | future config projection/query surface | mock only in prototype |

## Visual Direction

This version should feel like a calm operator workspace, not a dramatic dashboard.

- light canvas
- quiet borders
- compact typography
- sparse accent color
- stronger page hierarchy than the previous proposal
- theme system must have both white and black variants
- theme switching must not change semantics: user/tool/success/failure colors stay recognizable across both themes
- semantic cards should carry color mostly through border, title row, and badges, not through heavy full-card fills
- implementation theme assets are separated from WebUI layout and protocol-consumer assets:
  - `apps/freehand-server/assets/theme.css`
  - `apps/freehand-server/assets/theme.js`
  - `apps/freehand-server/assets/webui.css`
  - `apps/freehand-server/assets/webui.js`

## Prototype Scope

### Included Now

- workspace page
- settings page
- desktop shell
- mobile collapse rules in CSS

### Not Claimed Yet

- real runtime data binding
- real settings persistence
- full nodes page
- full replay page
- full debug page

## Review Questions

1. Default landing page should be `Workspace` only, correct?
2. `Settings` should stay fully separated from runtime workspace, correct?
3. On phone, do you want the command composer above the stream as proposed, or anchored to the bottom with an expand action?
4. Should `Slave` stay as an inspector tab, or become a separate workspace subpage?
