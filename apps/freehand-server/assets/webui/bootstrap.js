import { initializeThemeToggle } from "../theme.js?v=__WEBUI_ASSET_VERSION__";
import { classifyLayoutShape, classifyLayoutShapeForClient, viewportDimensionsForLayout } from "./app-shell/layout-shape.js?v=__WEBUI_ASSET_VERSION__";
import { webuiEdges, requireEdge, WebUiSurface } from "./app-shell/edge-registry.js?v=__WEBUI_ASSET_VERSION__";
import { createRouteController } from "./app-shell/route-controller.js?v=__WEBUI_ASSET_VERSION__";
import { sharedStateContract } from "./app-shell/shared-states/index.js?v=__WEBUI_ASSET_VERSION__";
import { surfaceContracts } from "./app-shell/surface-registry.js?v=__WEBUI_ASSET_VERSION__";

export async function initializeMobileWebui() {
  window.__freehandWebUiEdges = webuiEdges;
  window.__freehandWebUiSurface = WebUiSurface;
  window.__freehandWebUiSurfaceContracts = surfaceContracts;
  window.__freehandRequireWebUiEdge = requireEdge;
  window.__freehandCreateRouteController = createRouteController;
  window.__freehandSharedStateContract = sharedStateContract;
  window.__freehandLayout = {
    ...(window.__freehandLayout || {}),
    classifyLayoutShape,
    classifyLayoutShapeForClient,
    viewportDimensionsForLayout,
  };
  initializeThemeToggle(document);
  await import("./legacy-monolith.js?v=__WEBUI_ASSET_VERSION__");
}
