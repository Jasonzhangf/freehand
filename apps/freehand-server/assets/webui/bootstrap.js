import { initializeThemeToggle } from "/assets/theme.js?v=__WEBUI_ASSET_VERSION__";
import { classifyLayoutShape, classifyLayoutShapeForClient, viewportDimensionsForLayout } from "./app-shell/layout-shape.js?v=__WEBUI_ASSET_VERSION__";
import { webuiEdges, requireEdge, WebUiSurface } from "./app-shell/edge-registry.js?v=__WEBUI_ASSET_VERSION__";
import { createRouteController } from "./app-shell/route-controller.js?v=__WEBUI_ASSET_VERSION__";
import { surfaceContract as homeDashboardSurface } from "./surfaces/home-dashboard/index.js?v=__WEBUI_ASSET_VERSION__";
import { surfaceContract as sessionDetailSurface } from "./surfaces/session-detail/index.js?v=__WEBUI_ASSET_VERSION__";
import { surfaceContract as toolsRegistrySurface } from "./surfaces/tools-registry/index.js?v=__WEBUI_ASSET_VERSION__";
import { surfaceContract as timerDashboardSurface } from "./surfaces/timer-dashboard/index.js?v=__WEBUI_ASSET_VERSION__";
import { surfaceContract as settingsSurface } from "./surfaces/settings/index.js?v=__WEBUI_ASSET_VERSION__";
import { surfaceContract as sessionSearchSurface } from "./surfaces/session-search/index.js?v=__WEBUI_ASSET_VERSION__";
import { surfaceContract as newSessionSurface } from "./surfaces/new-session/index.js?v=__WEBUI_ASSET_VERSION__";

const surfaceContracts = Object.freeze([
  homeDashboardSurface,
  sessionDetailSurface,
  toolsRegistrySurface,
  timerDashboardSurface,
  settingsSurface,
  sessionSearchSurface,
  newSessionSurface,
]);

export async function initializeMobileWebui() {
  window.__freehandWebUiEdges = webuiEdges;
  window.__freehandWebUiSurface = WebUiSurface;
  window.__freehandWebUiSurfaceContracts = surfaceContracts;
  window.__freehandRequireWebUiEdge = requireEdge;
  window.__freehandCreateRouteController = createRouteController;
  window.__freehandLayout = {
    ...(window.__freehandLayout || {}),
    classifyLayoutShape,
    classifyLayoutShapeForClient,
    viewportDimensionsForLayout,
  };
  initializeThemeToggle(document);
  await import("./legacy-monolith.js?v=__WEBUI_ASSET_VERSION__");
}
