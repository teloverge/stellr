use std::{future::Future, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::{
        Request, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use stellr_core::Model;
use subtle::ConstantTimeEq;

use crate::state::AppState;

const TOKEN_COOKIE: &str = "stellr_token";
const CONTROL_SEND_DEADLINE: Duration = Duration::from_secs(1);

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/model", get(model))
        .route("/ws/control", get(control_ws))
        .layer(middleware::from_fn_with_state(state.clone(), auth))
        .with_state(state)
}

async fn model(State(state): State<Arc<AppState>>) -> Json<Model> {
    Json(state.hub.borrow().clone())
}

async fn control_ws(State(state): State<Arc<AppState>>, ws: WebSocketUpgrade) -> impl IntoResponse {
    let receiver = state.hub.subscribe();
    ws.on_upgrade(move |socket| control_loop(socket, receiver))
}

async fn control_loop(mut socket: WebSocket, mut receiver: tokio::sync::watch::Receiver<Model>) {
    if !send_snapshot(&mut socket, &mut receiver).await {
        return;
    }

    loop {
        tokio::select! {
            changed = receiver.changed() => {
                if changed.is_err() {
                    let _ = send_watch_closure(socket.send(Message::Close(None))).await;
                    return;
                }
                if !send_snapshot(&mut socket, &mut receiver).await {
                    return;
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    None | Some(Err(_)) => return,
                    Some(Ok(Message::Close(_))) => {
                        // One more read lets tungstenite flush its automatic close reply.
                        let _ = socket.recv().await;
                        return;
                    }
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}

async fn send_snapshot(
    socket: &mut WebSocket,
    receiver: &mut tokio::sync::watch::Receiver<Model>,
) -> bool {
    let Ok(snapshot) = serde_json::to_string(&*receiver.borrow_and_update()) else {
        return false;
    };
    send_with_deadline(socket.send(Message::Text(snapshot.into()))).await
}

async fn send_with_deadline<F, E>(send: F) -> bool
where
    F: Future<Output = Result<(), E>>,
{
    matches!(
        tokio::time::timeout(CONTROL_SEND_DEADLINE, send).await,
        Ok(Ok(()))
    )
}

async fn send_watch_closure<F, E>(close: F) -> bool
where
    F: Future<Output = Result<(), E>>,
{
    send_with_deadline(close).await
}

async fn auth(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    let Some(expected) = state.token.as_deref() else {
        return next.run(request).await;
    };

    if query_token_is_valid(request.uri().query(), expected) {
        let mut response = next.run(request).await;
        let cookie = format!("{TOKEN_COOKIE}={expected}; HttpOnly; SameSite=Strict; Path=/");
        let Ok(cookie) = HeaderValue::from_str(&cookie) else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        response.headers_mut().append(header::SET_COOKIE, cookie);
        return response;
    }

    if cookie_token_is_valid(request.headers(), expected)
        || bearer_token_is_valid(request.headers(), expected)
    {
        return next.run(request).await;
    }

    StatusCode::UNAUTHORIZED.into_response()
}

fn query_token_is_valid(query: Option<&str>, expected: &str) -> bool {
    let Some(query) = query else {
        return false;
    };

    let mut tokens = url::form_urlencoded::parse(query.as_bytes())
        .filter_map(|(name, value)| (name == "token").then_some(value));
    matches!(tokens.next(), Some(token) if token_matches(&token, expected))
        && tokens.next().is_none()
}

fn cookie_token_is_valid(headers: &HeaderMap, expected: &str) -> bool {
    let mut tokens = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|header| header.to_str().ok())
        .flat_map(|cookies| cookies.split(';'))
        .filter_map(|cookie| {
            let (name, value) = cookie.trim().split_once('=')?;
            (name == TOKEN_COOKIE).then_some(value)
        });

    matches!(tokens.next(), Some(token) if token_matches(token, expected))
        && tokens.next().is_none()
}

fn bearer_token_is_valid(headers: &HeaderMap, expected: &str) -> bool {
    let mut values = headers
        .get_all(header::AUTHORIZATION)
        .iter()
        .filter_map(|header| header.to_str().ok());
    let Some(value) = values.next() else {
        return false;
    };
    if values.next().is_some() {
        return false;
    }

    let mut parts = value.split_ascii_whitespace();
    let (Some(scheme), Some(token)) = (parts.next(), parts.next()) else {
        return false;
    };
    scheme.eq_ignore_ascii_case("Bearer")
        && parts.next().is_none()
        && token_matches(token, expected)
}

fn token_matches(candidate: &str, expected: &str) -> bool {
    candidate.len() == expected.len() && bool::from(candidate.as_bytes().ct_eq(expected.as_bytes()))
}

#[cfg(test)]
mod tests {
    use std::future;

    use super::{send_watch_closure, send_with_deadline};

    #[tokio::test]
    async fn send_deadline_drops_a_stalled_send() {
        assert!(!send_with_deadline(future::pending::<Result<(), ()>>()).await);
    }

    #[tokio::test]
    async fn watch_closure_close_is_deadline_bounded() {
        assert!(!send_watch_closure(future::pending::<Result<(), ()>>()).await);
    }
}
