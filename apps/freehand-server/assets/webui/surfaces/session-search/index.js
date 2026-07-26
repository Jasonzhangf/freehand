import { renderSessionSearchResult, renderSessionSearchSurface } from './view.js?v=20260726-header-worker-rail';

export const surfaceId = 'session_search';

export const surfaceContract = Object.freeze({
  surfaceId,
  domRootId: 'session-search-dialog',
  role: 'owner_query_sheet',
  owns: Object.freeze(['session_search_query', 'session_search_results']),
  entryEdges: Object.freeze(['home.open_search']),
  exitEdges: Object.freeze(['search.open_result']),
  forbiddenResponsibilities: Object.freeze(['browser_local_session_search', 'promote_worker_to_top_level']),
});

export function surfaceRoot(documentRef = document) {
  return documentRef.getElementById(surfaceContract.domRootId);
}

export function renderSurface(context) {
  renderSessionSearchSurface(context);
}

export { renderSessionSearchResult };
