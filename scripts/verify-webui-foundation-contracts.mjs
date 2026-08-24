import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';

import {
  SharedUiStateKind,
  createSharedStateModel,
} from '../apps/freehand-server/assets/webui/app-shell/shared-states/model.js';
import {
  renderSharedState,
} from '../apps/freehand-server/assets/webui/app-shell/shared-states/view.js';
import {
  surfaceContracts,
  validateSurfaceContractRegistry,
} from '../apps/freehand-server/assets/webui/app-shell/surface-registry.js';
import {
  settleAdpResponseFrame,
} from '../apps/freehand-server/assets/webui/app-shell/adp-client.js';
import {
  bindAnimatedDialogCancel,
  closeAnimatedDialog,
  openAnimatedDialog,
} from '../apps/freehand-server/assets/webui/app-shell/dialog-motion.js';

const root = new URL('../', import.meta.url);
const bootstrap = await readFile(
  new URL('apps/freehand-server/assets/webui/bootstrap.js', root),
  'utf8',
);
const adpClient = await readFile(
  new URL('apps/freehand-server/assets/webui/app-shell/adp-client.js', root),
  'utf8',
);
const homeView = await readFile(
  new URL('apps/freehand-server/assets/webui/surfaces/home-dashboard/view.js', root),
  'utf8',
);
const legacyWebui = await readFile(
  new URL('apps/freehand-server/assets/webui/legacy-monolith.js', root),
  'utf8',
);
const webuiCss = await readFile(
  new URL('apps/freehand-server/assets/webui.css', root),
  'utf8',
);
const serverAssets = await readFile(
  new URL('apps/freehand-server/src/assets.rs', root),
  'utf8',
);
const makefile = await readFile(new URL('Makefile', root), 'utf8');
const onlineVerifier = await readFile(
  new URL('scripts/verify-webui-mobile-ui-tree-online.mjs', root),
  'utf8',
);

const registeredSurfaces = surfaceContracts.map(({ surfaceId }) => surfaceId);
assert.deepEqual(registeredSurfaces, [
  'home_dashboard',
  'session_detail',
  'tools_registry',
  'timer_dashboard',
  'settings',
  'session_search',
  'new_session',
]);
assert(Object.isFrozen(surfaceContracts));
assert.equal(new Set(surfaceContracts.map(({ surfaceId }) => surfaceId)).size, surfaceContracts.length);
assert.equal(new Set(surfaceContracts.map(({ domRootId }) => domRootId)).size, surfaceContracts.length);
assert.throws(() => validateSurfaceContractRegistry({}), /must not be empty/);
assert.throws(
  () => validateSurfaceContractRegistry({
    invalid: Object.freeze({
      surfaceId: 'invalid',
      domRootId: '',
      role: 'test',
      owns: Object.freeze(['state']),
      entryEdges: Object.freeze(['root.open']),
      exitEdges: Object.freeze(['root.close']),
      forbiddenResponsibilities: Object.freeze(['truth']),
    }),
  }),
  /domRootId must be a non-empty string/,
);
assert.throws(
  () => validateSurfaceContractRegistry({
    invalid: Object.freeze({
      surfaceId: 'invalid',
      domRootId: 'invalid-root',
      role: 'test',
      owns: Object.freeze(['']),
      entryEdges: Object.freeze(['root.open']),
      exitEdges: Object.freeze(['root.close']),
      forbiddenResponsibilities: Object.freeze(['truth']),
    }),
  }),
  /owns must contain non-empty strings/,
);
assert.throws(
  () => validateSurfaceContractRegistry({
    invalid: Object.freeze({
      surfaceId: 'invalid',
      domRootId: 'invalid-root',
      role: 42,
      owns: Object.freeze(['state']),
      entryEdges: Object.freeze(['root.open']),
      exitEdges: Object.freeze(['root.close']),
      forbiddenResponsibilities: Object.freeze(['truth']),
    }),
  }),
  /role must be a non-empty string/,
);
assert.throws(
  () => validateSurfaceContractRegistry({
    first: Object.freeze({ ...surfaceContracts[0], surfaceId: 'first' }),
    second: Object.freeze({ ...surfaceContracts[1], surfaceId: 'second', domRootId: surfaceContracts[0].domRootId }),
  }),
  /duplicate surface DOM root/,
);
assert.throws(
  () => validateSurfaceContractRegistry({
    first: surfaceContracts[0],
  }),
  /registry key first does not match/,
);

assert.match(bootstrap, /app-shell\/surface-registry\.js/);
assert.match(bootstrap, /__freehandWebUiSurfaceContracts = surfaceContracts/);
assert.match(bootstrap, /__freehandSharedStateContract = sharedStateContract/);
assert.match(adpClient, /generated\/adp-protocol\.js/);
assert.match(adpClient, /ADP_PROTOCOL_VERSION/);
assert.match(adpClient, /frame\.protocol_version !== ADP_PROTOCOL_VERSION/);
assert.match(homeView, /delete list\.dataset\.sharedState/);
assert.match(legacyWebui, /typeof loaded !== "boolean"/);
assert.match(legacyWebui, /!Array\.isArray\(sessions\)/);
assert.match(onlineVerifier, /async function productionAssetVersion\(\)/);
assert.doesNotMatch(onlineVerifier, /const assetVersion = ['"][^'"]+['"]/);
assert.match(onlineVerifier, /runningHomeClearsSharedActiveState/);
assert.match(webuiCss, /--surface-open-duration:\s*210ms/);
assert.match(webuiCss, /--surface-close-duration:\s*150ms/);
assert.match(webuiCss, /--surface-scale-from:\s*0\.96/);
assert.match(webuiCss, /@media \(prefers-reduced-motion: reduce\)/);
assert.match(webuiCss, /transition: none !important/);
assert.match(serverAssets, /"webui\/app-shell\/dialog-motion\.js"/);
assert.match(serverAssets, /20260824-webui-surface-motion/);

{
  const moduleRegistry = JSON.parse(await readFile(
    new URL('docs/module-registry/app.webui-smoke.json', root),
    'utf8',
  ));
  const verificationMap = JSON.parse(await readFile(
    new URL('docs/verification-maps/app.webui-smoke.json', root),
    'utf8',
  ));

  assert.equal(moduleRegistry.schema_version, 1);
  assert.equal(moduleRegistry.registry_id, 'app.webui-smoke.modules');
  assert.equal(moduleRegistry.feature_id, 'app.webui-smoke');
  assert.equal(moduleRegistry.status, 'active');
  assert.deepEqual(moduleRegistry.coverage_roots, ['apps/freehand-server']);
  assert.ok(moduleRegistry.modules.every(({ status }) => status === 'active'));

  const ownedPaths = moduleRegistry.modules.flatMap(({ owned_paths }) => owned_paths);
  assert.equal(new Set(ownedPaths).size, ownedPaths.length);
  assert.deepEqual(ownedPaths.sort(), (await listRepositoryFiles('apps/freehand-server')).sort());
  assert.deepEqual(
    moduleRegistry.declared_edges,
    [{
      edge_id: 'app.webui-smoke.server-boundary_to_presentation-shell',
      from_module_id: 'app.webui-smoke.server-boundary',
      to_module_id: 'app.webui-smoke.presentation-shell',
      import_name: 'asset_response embedded served assets',
    }],
  );

  assert.equal(verificationMap.schema_version, 1);
  assert.equal(verificationMap.verification_map_id, 'app.webui-smoke.verification');
  assert.equal(verificationMap.feature_id, 'app.webui-smoke');
  assert.equal(verificationMap.status, 'active');
  assert.equal(verificationMap.module_registry, 'docs/module-registry/app.webui-smoke.json');
  assert.equal(verificationMap.function_map, 'docs/function-maps/app.webui-smoke.md');
  assert.equal(verificationMap.mainline_call_map, 'docs/mainline-calls/app.webui-smoke.json');
  assert.equal(verificationMap.test_design, 'docs/testing/app.webui-smoke.md');

  const activeGates = new Map(verificationMap.gates.map((gate) => [gate.gate_id, gate]));
  for (const [gateId, expectedCommandPart] of [
    ['app.webui-smoke.foundation-contracts', 'node scripts/verify-webui-foundation-contracts.mjs'],
    ['app.webui-smoke.unit', 'cargo test -p freehand-server'],
    ['app.webui-smoke.clippy', 'cargo clippy -p freehand-server'],
    ['app.webui-smoke.architecture', 'cargo run -p xtask -- gates check'],
    ['app.webui-smoke.surface-motion-online', 'node scripts/verify-webui-surface-motion-online.mjs'],
  ]) {
    const gate = activeGates.get(gateId);
    assert.ok(gate, `missing ${gateId}`);
    assert.equal(gate.binding_status, 'active');
    assert.match(gate.command, new RegExp(expectedCommandPart.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
  }
  assert.match(makefile, /^\.PHONY:.*verify-webui-surface-motion-online(?:\s|$)/m);
  assert.match(
    makefile,
    /^nightly: ci verify-webui-online verify-webui-surface-motion-online verify-webui-release-online$/m,
  );
}

function dialogStub({ initiallyOpen = false } = {}) {
  const classes = new Set();
  return {
    open: initiallyOpen,
    closeCount: 0,
    showModalCount: 0,
    listeners: {},
    classList: {
      add: (name) => classes.add(name),
      remove: (name) => classes.delete(name),
      contains: (name) => classes.has(name),
    },
    showModal() {
      this.showModalCount += 1;
      this.open = true;
    },
    close() {
      if (!this.open) return;
      this.open = false;
      this.closeCount += 1;
    },
    addEventListener(name, handler) {
      this.listeners[name] = handler;
    },
  };
}

function windowStub() {
  let nextId = 1;
  return {
    timeouts: [],
    clearedTimeoutIds: [],
    animationFrames: [],
    setTimeout(callback, delayMs) {
      const timeoutId = nextId++;
      this.timeouts.push({ timeoutId, callback, delayMs, cancelled: false });
      return timeoutId;
    },
    clearTimeout(timeoutId) {
      this.clearedTimeoutIds.push(timeoutId);
      const timeout = this.timeouts.find((entry) => entry.timeoutId === timeoutId);
      if (timeout) timeout.cancelled = true;
    },
    requestAnimationFrame(callback) {
      this.animationFrames.push(callback);
      return this.animationFrames.length;
    },
  };
}

{
  const dialog = dialogStub();
  const animationWindow = windowStub();
  assert.equal(openAnimatedDialog(dialog, animationWindow), true);
  assert.equal(dialog.showModalCount, 1);
  animationWindow.animationFrames[0]();
  assert.equal(dialog.classList.contains('is-open'), true);

  const closeCallbacks = [];
  closeAnimatedDialog(dialog, () => closeCallbacks.push('closed'), animationWindow);
  assert.equal(dialog.classList.contains('is-closing'), true);
  assert.deepEqual(animationWindow.timeouts.map(({ delayMs }) => delayMs), [150]);
  if (!animationWindow.timeouts[0].cancelled) animationWindow.timeouts[0].callback();
  assert.equal(dialog.closeCount, 1);
  assert.deepEqual(closeCallbacks, ['closed']);
}

{
  const dialog = dialogStub({ initiallyOpen: true });
  const animationWindow = windowStub();
  const closeCallbacks = [];
  openAnimatedDialog(dialog, animationWindow);
  closeAnimatedDialog(dialog, () => closeCallbacks.push('stale-close'), animationWindow);
  openAnimatedDialog(dialog, animationWindow);
  assert.deepEqual(animationWindow.clearedTimeoutIds, [animationWindow.timeouts[0].timeoutId]);
  if (!animationWindow.timeouts[0].cancelled) animationWindow.timeouts[0].callback();
  assert.equal(dialog.open, true);
  assert.equal(dialog.closeCount, 0);
  assert.deepEqual(closeCallbacks, []);
  animationWindow.animationFrames.at(-1)();
  assert.equal(dialog.classList.contains('is-open'), true);
}

{
  const dialog = dialogStub({ initiallyOpen: true });
  const animationWindow = windowStub();
  const closeCallbacks = [];
  closeAnimatedDialog(dialog, () => closeCallbacks.push('close'), animationWindow);
  closeAnimatedDialog(dialog, () => closeCallbacks.push('duplicate'), animationWindow);
  assert.equal(animationWindow.timeouts.length, 1);
  animationWindow.timeouts[0].callback();
  assert.deepEqual(closeCallbacks, ['close']);
  assert.equal(dialog.closeCount, 1);
}

{
  const dialog = dialogStub({ initiallyOpen: true });
  const animationWindow = windowStub();
  const observed = [];
  bindAnimatedDialogCancel(
    dialog,
    () => observed.push('surface-close'),
    animationWindow,
  );
  dialog.listeners.cancel({ preventDefault: () => observed.push('native-cancel') });
  animationWindow.timeouts[0].callback();
  assert.deepEqual(observed, ['native-cancel', 'surface-close']);
  assert.equal(dialog.closeCount, 1);
}

function settlementState(requestId, callbacks) {
  return {
    adpRequests: new Map([[requestId, {
      timeoutId: `timeout-${requestId}`,
      resolve: (value) => callbacks.push({ type: 'resolve', value }),
      reject: (error) => callbacks.push({ type: 'reject', value: error.message }),
    }]]),
    adpSubscriptions: new Set(),
  };
}

const clearedTimeouts = [];
const settlementWindow = {
  clearTimeout: (timeoutId) => clearedTimeouts.push(timeoutId),
};
const queryCallbacks = [];
const queryState = settlementState('query-1', queryCallbacks);
assert.deepEqual(
  settleAdpResponseFrame({
    state: queryState,
    windowRef: settlementWindow,
    frame: { kind: 'query_result', request_id: 'query-1', result: { sessions: [] } },
  }),
  { kind: 'query_result', settled: true },
);
assert.deepEqual(queryCallbacks, [{ type: 'resolve', value: { sessions: [] } }]);
assert.equal(queryState.adpRequests.size, 0);

const commandCallbacks = [];
const commandState = settlementState('command-1', commandCallbacks);
const commandReceipt = { dispatch_status: 'accepted' };
assert.deepEqual(
  settleAdpResponseFrame({
    state: commandState,
    windowRef: settlementWindow,
    frame: { kind: 'command_receipt', request_id: 'command-1', receipt: commandReceipt },
  }),
  { kind: 'command_receipt', settled: true, receipt: commandReceipt },
);
assert.deepEqual(commandCallbacks, [{ type: 'resolve', value: commandReceipt }]);

const subscribeCallbacks = [];
const subscribeState = settlementState('subscribe-1', subscribeCallbacks);
const selector = { stream_kind: 'latest_turn' };
assert.deepEqual(
  settleAdpResponseFrame({
    state: subscribeState,
    windowRef: settlementWindow,
    frame: { kind: 'subscription_accepted', request_id: 'subscribe-1', selector },
  }),
  { kind: 'subscription_accepted', settled: true, selector },
);
assert.deepEqual(subscribeCallbacks, [{ type: 'resolve', value: selector }]);
assert(subscribeState.adpSubscriptions.has('subscribe-1'));

const eventState = { adpRequests: new Map(), adpSubscriptions: new Set() };
const event = { stream_kind: 'latest_turn', payload: { status: 'running' } };
assert.deepEqual(
  settleAdpResponseFrame({
    state: eventState,
    windowRef: settlementWindow,
    frame: { kind: 'subscription_event', request_id: 'subscribe-1', event },
  }),
  { kind: 'subscription_event', settled: false, event },
);

const failureCallbacks = [];
const failureState = settlementState('query-2', failureCallbacks);
const failure = { code: 'query_failed', message: 'query rejected' };
assert.deepEqual(
  settleAdpResponseFrame({
    state: failureState,
    windowRef: settlementWindow,
    frame: { kind: 'failure', request_id: 'query-2', failure },
  }),
  { kind: 'failure', settled: true, failure },
);
assert.deepEqual(failureCallbacks, [{ type: 'reject', value: 'query rejected' }]);
const malformedFailureCallbacks = [];
const malformedFailureState = settlementState('query-malformed', malformedFailureCallbacks);
assert.throws(
  () => settleAdpResponseFrame({
    state: malformedFailureState,
    windowRef: settlementWindow,
    frame: { kind: 'failure', request_id: 'query-malformed' },
  }),
  /violates the generated protocol contract/,
);
assert.equal(malformedFailureState.adpRequests.size, 1);
assert.deepEqual(malformedFailureCallbacks, []);
assert.deepEqual(
  settleAdpResponseFrame({
    state: eventState,
    windowRef: settlementWindow,
    frame: { kind: 'unsupported', request_id: 'unknown-1' },
  }),
  { kind: 'unsupported', settled: false, unsupported: true },
);
assert.deepEqual(clearedTimeouts, [
  'timeout-query-1',
  'timeout-command-1',
  'timeout-subscribe-1',
  'timeout-query-2',
]);

const stateModels = Object.values(SharedUiStateKind).map((kind) =>
  createSharedStateModel(kind, {
    title: ` ${kind} title `,
    detail: ` ${kind} detail `,
    actionLabel: kind === SharedUiStateKind.Confirmation ? ' Confirm ' : '',
    actionId: kind === SharedUiStateKind.Confirmation ? 'confirm' : '',
  }),
);
for (const model of stateModels) {
  assert(Object.isFrozen(model));
  assert.equal(model.title, `${model.kind} title`);
  assert.equal(model.detail, `${model.kind} detail`);
}
assert.throws(
  () => createSharedStateModel('waiting'),
  /unsupported shared UI state/,
);
assert.throws(
  () => createSharedStateModel(SharedUiStateKind.Error),
  /requires fields/,
);
assert.throws(
  () => createSharedStateModel(SharedUiStateKind.Error, { title: '' }),
  /requires error\.title/,
);
assert.throws(
  () => createSharedStateModel(SharedUiStateKind.Confirmation, { title: 'Confirm' }),
  /requires an action/,
);

function element(tagName) {
  return {
    tagName,
    className: '',
    dataset: {},
    attributes: {},
    children: [],
    textContent: '',
    setAttribute(name, value) {
      this.attributes[name] = value;
    },
    append(...children) {
      this.children.push(...children);
    },
    addEventListener(name, handler) {
      this.listener = { name, handler };
    },
  };
}

const container = element('div');
container.ownerDocument = { createElement: element };
container.replaceChildren = function replaceChildren(...children) {
  this.children = children;
};
let actionId = null;
renderSharedState(
  container,
  createSharedStateModel(SharedUiStateKind.Confirmation, {
    title: 'Confirm',
    detail: 'Proceed',
    actionLabel: 'Continue',
    actionId: 'continue',
  }),
  { onAction: (value) => { actionId = value; } },
);
assert.equal(container.dataset.sharedState, 'confirmation');
assert.equal(container.children[0].attributes.role, 'status');
container.children[0].children[2].listener.handler();
assert.equal(actionId, 'continue');
assert.throws(() => renderSharedState(null, stateModels[0]), /requires a container/);
assert.throws(
  () => renderSharedState(
    container,
    createSharedStateModel(SharedUiStateKind.Confirmation, {
      title: 'Confirm',
      actionLabel: 'Continue',
      actionId: 'continue',
    }),
  ),
  /requires onAction/,
);

async function listRepositoryFiles(relativeRoot) {
  const directoryUrl = new URL(`${relativeRoot}/`, root);
  const entries = await readdir(directoryUrl, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const relativePath = `${relativeRoot}/${entry.name}`;
    if (entry.isDirectory()) {
      files.push(...await listRepositoryFiles(relativePath));
    } else if (entry.isFile()) {
      files.push(relativePath);
    }
  }
  return files.sort();
}

console.log(JSON.stringify({
  ok: true,
  registeredSurfaces,
  sharedStates: stateModels.map(({ kind }) => kind),
}));
