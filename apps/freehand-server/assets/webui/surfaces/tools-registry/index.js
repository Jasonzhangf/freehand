import { renderToolsRegistrySurface } from './view.js?v=20260726-session-select-rename';
import { openToolsRegistrySurface, refreshToolsRegistrySurface } from './controls.js?v=20260726-session-select-rename';

export const surfaceId = 'tools_registry';

export const surfaceContract = Object.freeze({
  surfaceId,
  domRootId: 'tools-dashboard-dialog',
  role: 'owner_projection_sheet',
  owns: Object.freeze(['tool_registry_projection']),
  entryEdges: Object.freeze(['root.open_tools']),
  exitEdges: Object.freeze(['tools.refresh']),
  forbiddenResponsibilities: Object.freeze(['execute_tool', 'store_tool_registry_locally']),
});

export function surfaceRoot(documentRef = document) {
  return documentRef.getElementById(surfaceContract.domRootId);
}

export function renderSurface(context) {
  renderToolsRegistrySurface(context);
}

export { openToolsRegistrySurface, refreshToolsRegistrySurface };
