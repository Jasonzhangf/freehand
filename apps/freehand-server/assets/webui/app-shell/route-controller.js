import { WebUiSurface, requireEdge } from "./edge-registry.js?v=__WEBUI_ASSET_VERSION__";

export function createRouteController({ state, document }) {
  function setRoute(route, params = {}) {
    const nextRoute = route || WebUiSurface.HomeDashboard;
    state.route = nextRoute;
    state.routeParams = { ...params };
    const body = document.body;
    const shell = document.querySelector("[data-webui-shell]");
    body.dataset.webuiRoute = nextRoute;
    if (shell) {
      shell.dataset.webuiRoute = nextRoute;
      shell.dataset.primarySurface = nextRoute;
      if (params.session_id) {
        shell.dataset.routeSession = params.session_id;
      } else {
        delete shell.dataset.routeSession;
      }
    }
  }

  function dispatch(edgeId, payload = {}) {
    const edge = requireEdge(edgeId, payload);
    switch (edgeId) {
      case "root.open_home":
      case "session.back_home":
        setRoute(WebUiSurface.HomeDashboard);
        break;
      case "home.open_session":
      case "session.open_parent_session":
      case "search.open_result":
        setRoute(WebUiSurface.SessionDetail, { session_id: payload.session_id });
        break;
      case "session.open_worker_session":
        setRoute(WebUiSurface.SessionDetail, { session_id: payload.worker_session_id });
        break;
      case "home.open_search":
        setRoute(WebUiSurface.SessionSearch);
        break;
      case "home.open_memory":
        setRoute(WebUiSurface.Memory);
        break;
      case "home.open_new":
        setRoute(WebUiSurface.NewSession);
        break;
      case "root.open_tools":
        setRoute(WebUiSurface.ToolsRegistry);
        break;
      case "root.open_timer":
        setRoute(WebUiSurface.TimerDashboard);
        break;
      case "root.open_settings":
        setRoute(WebUiSurface.Settings);
        break;
      case "new.created":
        setRoute(WebUiSurface.SessionDetail, { session_id: payload.session_id });
        break;
      default:
        break;
    }
    return edge;
  }

  return {
    setRoute,
    dispatch,
    currentRoute() {
      return state.route || WebUiSurface.HomeDashboard;
    },
  };
}
