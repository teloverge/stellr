use axum::{
    extract::Request,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../web/dist"]
struct Dist;

pub async fn static_handler(request: Request) -> Response {
    let path = request.uri().path().trim_start_matches('/');
    if is_server_path(path) {
        return StatusCode::NOT_FOUND.into_response();
    }

    let requested = if path.is_empty() { "index.html" } else { path };
    let (served_path, asset) = match Dist::get(requested) {
        Some(asset) => (requested, asset),
        None => match Dist::get("index.html") {
            Some(index) => ("index.html", index),
            None => return StatusCode::NOT_FOUND.into_response(),
        },
    };
    let content_type = mime_guess::from_path(served_path).first_or_octet_stream();

    (
        [(header::CONTENT_TYPE, content_type.as_ref())],
        asset.data.into_owned(),
    )
        .into_response()
}

fn is_server_path(path: &str) -> bool {
    path == "api" || path.starts_with("api/") || path == "ws" || path.starts_with("ws/")
}
