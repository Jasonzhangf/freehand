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

const root = new URL('../', import.meta.url);
const bootstrap = await readFile(
  new URL('apps/freehand-server/assets/webui/bootstrap.js', root),
  'utf8',
);
const adpClient = await readFile(
  new URL('apps/freehand-server/assets/webui/app-shell/adp-client.js', root),
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
