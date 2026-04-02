
fn app_http_user_agent() -> String {
    format!(
        "{}/{} ({}; tauri)",
        APP_HTTP_ORIGINATOR,
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS
    )
}

fn app_identity_headers() -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::HeaderName::from_static("originator"),
        reqwest::header::HeaderValue::from_static(APP_HTTP_ORIGINATOR),
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_str(&app_http_user_agent())
            .unwrap_or_else(|_| reqwest::header::HeaderValue::from_static(APP_HTTP_ORIGINATOR)),
    );
    headers
}

fn app_identity_rig_headers() -> rig::http_client::HeaderMap {
    let mut headers = rig::http_client::HeaderMap::new();
    headers.insert(
        "originator",
        rig::http_client::HeaderValue::from_static(APP_HTTP_ORIGINATOR),
    );
    headers.insert(
        "user-agent",
        rig::http_client::HeaderValue::from_str(&app_http_user_agent()).unwrap_or_else(|_| {
            rig::http_client::HeaderValue::from_static(APP_HTTP_ORIGINATOR)
        }),
    );
    headers
}
