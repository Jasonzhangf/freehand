import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

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

console.log(JSON.stringify({
  ok: true,
  registeredSurfaces,
  sharedStates: stateModels.map(({ kind }) => kind),
}));
