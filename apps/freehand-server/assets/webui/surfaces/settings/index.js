import { renderSettingsShellSurface, renderSettingsNavigationSurface } from './view.js?v=20260726-mobile-route-one-row';
import { renderDiagnosticLogRow, renderSettingsDiagnosticsSurface } from './diagnostics.js?v=20260726-mobile-route-one-row';

export const surfaceId = 'settings';

export const surfaceContract = Object.freeze({
  surfaceId,
  domRootId: 'settings-shell',
  role: 'owner_config_projection_sheet',
  owns: Object.freeze(['settings_navigation', 'config_status_projection', 'diagnostics_projection']),
  entryEdges: Object.freeze(['root.open_settings']),
  exitEdges: Object.freeze(['settings.navigate']),
  forbiddenResponsibilities: Object.freeze(['read_config_file', 'write_config_file', 'expose_secret']),
});

export function surfaceRoot(documentRef = document) {
  return documentRef.getElementById(surfaceContract.domRootId);
}

export function renderSurface(context) {
  renderSettingsShellSurface(context);
}

export function renderNavigation(context) {
  renderSettingsNavigationSurface(context);
}

export function renderDiagnostics(context) {
  renderSettingsDiagnosticsSurface(context);
}

export { renderDiagnosticLogRow };
