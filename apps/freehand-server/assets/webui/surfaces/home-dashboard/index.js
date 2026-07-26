import { createHomeDashboardModel } from './model.js?v=20260726-session-select-rename';
import { createHomeSessionRow } from './controls.js?v=20260726-session-select-rename';
import { renderHomeDashboard } from './view.js?v=20260726-session-select-rename';

export const surfaceId = 'home_dashboard';

export const surfaceContract = Object.freeze({
  surfaceId,
  domRootId: 'mobile-home-dashboard',
  role: 'primary_dashboard',
  owns: Object.freeze(['running_session_rows', 'history_session_rows', 'session_crud_controls']),
  entryEdges: Object.freeze(['root.open_home']),
  exitEdges: Object.freeze(['home.open_session', 'home.open_search', 'home.open_new', 'root.open_tools', 'root.open_timer', 'root.open_settings']),
  forbiddenResponsibilities: Object.freeze(['render_transcript', 'expand_worker_children_by_default', 'browser_local_session_truth']),
});

export function surfaceRoot(documentRef = document) {
  return documentRef.getElementById(surfaceContract.domRootId);
}

export function renderSurface(input, context) {
  const model = createHomeDashboardModel(input);
  renderHomeDashboard(model, {
    ...context,
    createSessionRow: (row) => createHomeSessionRow(row, context),
  });
  return model;
}
