use axum::body::Body;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

/// Single source of truth for the WebUI cache-busting version. Asset files and
/// the page template reference `__WEBUI_ASSET_VERSION__`; the server stamps
/// this value at serve time, so bumping the version is a one-line change here.
pub const WEBUI_ASSET_VERSION: &str = "20260730-openminis-foundation";

const WEBUI_ASSET_VERSION_TOKEN: &str = "__WEBUI_ASSET_VERSION__";

pub fn stamp_asset_version(body: &str) -> String {
    body.replace(WEBUI_ASSET_VERSION_TOKEN, WEBUI_ASSET_VERSION)
}

struct Asset {
    content_type: &'static str,
    body: &'static str,
}

const THEME_CSS: Asset = Asset {
    content_type: "text/css; charset=utf-8",
    body: include_str!("../assets/theme.css"),
};

const WEBUI_CSS: Asset = Asset {
    content_type: "text/css; charset=utf-8",
    body: include_str!("../assets/webui.css"),
};

const THEME_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/theme.js"),
};

const WEBUI_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui.js"),
};

const WEBUI_BOOTSTRAP_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/bootstrap.js"),
};

const WEBUI_LEGACY_MONOLITH_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/legacy-monolith.js"),
};

const WEBUI_LAYOUT_SHAPE_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/app-shell/layout-shape.js"),
};

const WEBUI_EDGE_REGISTRY_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/app-shell/edge-registry.js"),
};

const WEBUI_ROUTE_CONTROLLER_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/app-shell/route-controller.js"),
};

const WEBUI_SURFACE_REGISTRY_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/app-shell/surface-registry.js"),
};

const WEBUI_ADP_CLIENT_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/app-shell/adp-client.js"),
};

const WEBUI_SHARED_STATES_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/app-shell/shared-states/index.js"),
};

const WEBUI_SHARED_STATES_MODEL_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/app-shell/shared-states/model.js"),
};

const WEBUI_SHARED_STATES_VIEW_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/app-shell/shared-states/view.js"),
};

const WEBUI_ADP_PROTOCOL_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/generated/adp-protocol.js"),
};

const WEBUI_SURFACE_HOME_DASHBOARD_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/home-dashboard/index.js"),
};

const WEBUI_SURFACE_HOME_DASHBOARD_MODEL_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/home-dashboard/model.js"),
};

const WEBUI_SURFACE_HOME_DASHBOARD_VIEW_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/home-dashboard/view.js"),
};

const WEBUI_SURFACE_HOME_DASHBOARD_CONTROLS_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/home-dashboard/controls.js"),
};

const WEBUI_SURFACE_SESSION_DETAIL_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/session-detail/index.js"),
};

const WEBUI_SURFACE_SESSION_DETAIL_CONTROLS_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/session-detail/controls.js"),
};

const WEBUI_SURFACE_SESSION_DETAIL_RECOVERY_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/session-detail/recovery.js"),
};

const WEBUI_SURFACE_TOOLS_REGISTRY_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/tools-registry/index.js"),
};

const WEBUI_SURFACE_TOOLS_REGISTRY_VIEW_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/tools-registry/view.js"),
};

const WEBUI_SURFACE_TOOLS_REGISTRY_CONTROLS_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/tools-registry/controls.js"),
};

const WEBUI_SURFACE_TIMER_DASHBOARD_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/timer-dashboard/index.js"),
};

const WEBUI_SURFACE_TIMER_DASHBOARD_VIEW_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/timer-dashboard/view.js"),
};

const WEBUI_SURFACE_TIMER_DASHBOARD_CONTROLS_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/timer-dashboard/controls.js"),
};

const WEBUI_SURFACE_SETTINGS_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/settings/index.js"),
};

const WEBUI_SURFACE_SETTINGS_VIEW_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/settings/view.js"),
};

const WEBUI_SURFACE_SETTINGS_DIAGNOSTICS_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/settings/diagnostics.js"),
};

const WEBUI_SURFACE_SESSION_SEARCH_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/session-search/index.js"),
};

const WEBUI_SURFACE_SESSION_SEARCH_VIEW_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/session-search/view.js"),
};

const WEBUI_SURFACE_NEW_SESSION_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/new-session/index.js"),
};

const WEBUI_SURFACE_NEW_SESSION_CONTROLS_JS: Asset = Asset {
    content_type: "application/javascript; charset=utf-8",
    body: include_str!("../assets/webui/surfaces/new-session/controls.js"),
};

const LOGO_PNG: &[u8] = include_bytes!("../../../assets/logo.png");

pub fn asset_response(path: &str) -> Result<Response, StatusCode> {
    let asset = match path {
        "theme.css" => &THEME_CSS,
        "webui.css" => &WEBUI_CSS,
        "theme.js" => &THEME_JS,
        "webui.js" => &WEBUI_JS,
        "webui/bootstrap.js" => &WEBUI_BOOTSTRAP_JS,
        "webui/legacy-monolith.js" => &WEBUI_LEGACY_MONOLITH_JS,
        "webui/app-shell/layout-shape.js" => &WEBUI_LAYOUT_SHAPE_JS,
        "webui/app-shell/edge-registry.js" => &WEBUI_EDGE_REGISTRY_JS,
        "webui/app-shell/route-controller.js" => &WEBUI_ROUTE_CONTROLLER_JS,
        "webui/app-shell/surface-registry.js" => &WEBUI_SURFACE_REGISTRY_JS,
        "webui/app-shell/adp-client.js" => &WEBUI_ADP_CLIENT_JS,
        "webui/app-shell/shared-states/index.js" => &WEBUI_SHARED_STATES_JS,
        "webui/app-shell/shared-states/model.js" => &WEBUI_SHARED_STATES_MODEL_JS,
        "webui/app-shell/shared-states/view.js" => &WEBUI_SHARED_STATES_VIEW_JS,
        "webui/generated/adp-protocol.js" => &WEBUI_ADP_PROTOCOL_JS,
        "webui/surfaces/home-dashboard/index.js" => &WEBUI_SURFACE_HOME_DASHBOARD_JS,
        "webui/surfaces/home-dashboard/model.js" => &WEBUI_SURFACE_HOME_DASHBOARD_MODEL_JS,
        "webui/surfaces/home-dashboard/view.js" => &WEBUI_SURFACE_HOME_DASHBOARD_VIEW_JS,
        "webui/surfaces/home-dashboard/controls.js" => &WEBUI_SURFACE_HOME_DASHBOARD_CONTROLS_JS,
        "webui/surfaces/session-detail/index.js" => &WEBUI_SURFACE_SESSION_DETAIL_JS,
        "webui/surfaces/session-detail/controls.js" => &WEBUI_SURFACE_SESSION_DETAIL_CONTROLS_JS,
        "webui/surfaces/session-detail/recovery.js" => &WEBUI_SURFACE_SESSION_DETAIL_RECOVERY_JS,
        "webui/surfaces/tools-registry/index.js" => &WEBUI_SURFACE_TOOLS_REGISTRY_JS,
        "webui/surfaces/tools-registry/view.js" => &WEBUI_SURFACE_TOOLS_REGISTRY_VIEW_JS,
        "webui/surfaces/tools-registry/controls.js" => &WEBUI_SURFACE_TOOLS_REGISTRY_CONTROLS_JS,
        "webui/surfaces/timer-dashboard/index.js" => &WEBUI_SURFACE_TIMER_DASHBOARD_JS,
        "webui/surfaces/timer-dashboard/view.js" => &WEBUI_SURFACE_TIMER_DASHBOARD_VIEW_JS,
        "webui/surfaces/timer-dashboard/controls.js" => &WEBUI_SURFACE_TIMER_DASHBOARD_CONTROLS_JS,
        "webui/surfaces/settings/index.js" => &WEBUI_SURFACE_SETTINGS_JS,
        "webui/surfaces/settings/view.js" => &WEBUI_SURFACE_SETTINGS_VIEW_JS,
        "webui/surfaces/settings/diagnostics.js" => &WEBUI_SURFACE_SETTINGS_DIAGNOSTICS_JS,
        "webui/surfaces/session-search/index.js" => &WEBUI_SURFACE_SESSION_SEARCH_JS,
        "webui/surfaces/session-search/view.js" => &WEBUI_SURFACE_SESSION_SEARCH_VIEW_JS,
        "webui/surfaces/new-session/index.js" => &WEBUI_SURFACE_NEW_SESSION_JS,
        "webui/surfaces/new-session/controls.js" => &WEBUI_SURFACE_NEW_SESSION_CONTROLS_JS,
        "logo.png" => {
            return Response::builder()
                .header(header::CONTENT_TYPE, HeaderValue::from_static("image/png"))
                .header(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("no-store, max-age=0"),
                )
                .body(Body::from(LOGO_PNG))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
        }
        _ => return Err(StatusCode::NOT_FOUND),
    };
    Ok((
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(asset.content_type),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("no-store, max-age=0"),
            ),
        ],
        stamp_asset_version(asset.body),
    )
        .into_response())
}
