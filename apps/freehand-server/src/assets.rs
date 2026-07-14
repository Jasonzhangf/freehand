use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

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

pub fn asset_response(path: &str) -> Result<Response, StatusCode> {
    let asset = match path {
        "theme.css" => &THEME_CSS,
        "webui.css" => &WEBUI_CSS,
        "theme.js" => &THEME_JS,
        "webui.js" => &WEBUI_JS,
        _ => return Err(StatusCode::NOT_FOUND),
    };
    Ok((
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static(asset.content_type),
        )],
        asset.body,
    )
        .into_response())
}
