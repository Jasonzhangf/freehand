import { renderMemorySurface, renderMemoryEntry } from './view.js?v=__WEBUI_ASSET_VERSION__';

export const surfaceId = 'memory';

export const surfaceContract = Object.freeze({
  surfaceId,
  domRootId: 'memory-dialog',
  role: 'owner_query_sheet',
  owns: Object.freeze(['memory_query', 'memory_sort', 'memory_results']),
  entryEdges: Object.freeze(['home.open_memory']),
  exitEdges: Object.freeze(['memory.close']),
  forbiddenResponsibilities: Object.freeze([
    'browser_local_memory_search',
    'direct_filesystem_write',
    'session_truth_mutation',
  ]),
});

export function surfaceRoot(documentRef = document) {
  return documentRef.getElementById(surfaceContract.domRootId);
}

export function renderSurface(context) {
  renderMemorySurface(context);
}

export { renderMemoryEntry };
