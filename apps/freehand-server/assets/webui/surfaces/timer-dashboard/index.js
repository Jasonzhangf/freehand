import { renderTimerDashboardSurface } from './view.js?v=20260726-header-worker-rail';
import { cancelTimerFromSurface, openTimerDashboardSurface, refreshTimerDashboardSurface, scheduleTimerFromSurface } from './controls.js?v=20260726-header-worker-rail';

export const surfaceId = 'timer_dashboard';

export const surfaceContract = Object.freeze({
  surfaceId,
  domRootId: 'timer-dashboard-dialog',
  role: 'owner_projection_sheet',
  owns: Object.freeze(['timer_list_projection', 'timer_schedule_command']),
  entryEdges: Object.freeze(['root.open_timer']),
  exitEdges: Object.freeze(['timer.refresh']),
  forbiddenResponsibilities: Object.freeze(['browser_local_timer_truth', 'task_center_timer_fallback']),
});

export function surfaceRoot(documentRef = document) {
  return documentRef.getElementById(surfaceContract.domRootId);
}

export function renderSurface(context) {
  renderTimerDashboardSurface(context);
}

export { cancelTimerFromSurface, openTimerDashboardSurface, refreshTimerDashboardSurface, scheduleTimerFromSurface };
