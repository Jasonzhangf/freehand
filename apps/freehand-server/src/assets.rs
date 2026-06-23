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

const MOCK_ANDROID_HTML: Asset = Asset {
    content_type: "text/html; charset=utf-8",
    body: include_str!("../assets/mocks/android/mobile-mock.html"),
};

const MOCK_ANDROID_CSS: Asset = Asset {
    content_type: "text/css; charset=utf-8",
    body: include_str!("../assets/mocks/android/mobile-mock.css"),
};

pub fn asset_response(path: &str) -> Result<Response, StatusCode> {
    let asset = match path {
        "theme.css" => &THEME_CSS,
        "webui.css" => &WEBUI_CSS,
        "theme.js" => &THEME_JS,
        "webui.js" => &WEBUI_JS,
        "mocks/android/mobile-mock.html" => &MOCK_ANDROID_HTML,
        "mocks/android/mobile-mock.css" => &MOCK_ANDROID_CSS,
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
