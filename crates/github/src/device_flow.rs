//! Native-owned GitHub OAuth device authorization.

use std::{fmt, ops::Deref, sync::Arc, time::Duration};

use reqwest::{Client, Url, header::ACCEPT};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::Mutex,
    task::AbortHandle,
    time::{Instant, sleep},
};

const DEVICE_CODE_PATH: &str = "/login/device/code";
const ACCESS_TOKEN_PATH: &str = "/login/oauth/access_token";
const DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

#[derive(Debug, thiserror::Error)]
pub enum DeviceFlowError {
    #[error("invalid GitHub device-flow endpoint: {0}")]
    InvalidEndpoint(String),
    #[error("GitHub device authorization request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("GitHub device authorization returned {0}")]
    HttpStatus(reqwest::StatusCode),
    #[error("GitHub device authorization failed: {0}")]
    Api(String),
}

/// An access token whose debug representation can never reveal its contents.
#[derive(Clone, PartialEq, Eq)]
pub struct AccessToken(String);

impl AccessToken {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl Deref for AccessToken {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.expose()
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AccessToken([REDACTED])")
    }
}

#[derive(Clone)]
pub struct DeviceFlowClient {
    http: Client,
    base_url: Url,
    client_id: Arc<str>,
    scope: Arc<str>,
}

impl DeviceFlowClient {
    pub fn new(
        base_url: Url,
        client_id: impl Into<Arc<str>>,
        scope: impl Into<Arc<str>>,
    ) -> Result<Self, DeviceFlowError> {
        // Validate both paths here so construction fails before any authorization attempt.
        base_url
            .join(DEVICE_CODE_PATH)
            .map_err(|error| DeviceFlowError::InvalidEndpoint(error.to_string()))?;
        base_url
            .join(ACCESS_TOKEN_PATH)
            .map_err(|error| DeviceFlowError::InvalidEndpoint(error.to_string()))?;
        Ok(Self {
            http: Client::new(),
            base_url,
            client_id: client_id.into(),
            scope: scope.into(),
        })
    }

    pub async fn request_code(&self) -> Result<DeviceAuthorization, DeviceFlowError> {
        let response = self
            .http
            .post(
                self.base_url
                    .join(DEVICE_CODE_PATH)
                    .map_err(|error| DeviceFlowError::InvalidEndpoint(error.to_string()))?,
            )
            .header(ACCEPT, "application/json")
            .form(&[
                ("client_id", self.client_id.as_ref()),
                ("scope", self.scope.as_ref()),
            ])
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(DeviceFlowError::HttpStatus(status));
        }
        let response: DeviceCodeResponse = response.json().await?;
        Ok(DeviceAuthorization {
            device_code: response.device_code,
            user_code: response.user_code,
            verification_uri: response.verification_uri,
            expires_in_seconds: response.expires_in,
            interval_seconds: response.interval,
        })
    }

    pub async fn poll_once(
        &self,
        authorization: &DeviceAuthorization,
    ) -> Result<PollOutcome, DeviceFlowError> {
        let response = self
            .http
            .post(
                self.base_url
                    .join(ACCESS_TOKEN_PATH)
                    .map_err(|error| DeviceFlowError::InvalidEndpoint(error.to_string()))?,
            )
            .header(ACCEPT, "application/json")
            .form(&[
                ("client_id", self.client_id.as_ref()),
                ("device_code", authorization.device_code.as_str()),
                ("grant_type", DEVICE_GRANT_TYPE),
            ])
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(DeviceFlowError::HttpStatus(status));
        }
        let response: TokenResponse = response.json().await?;
        if let Some(token) = response.access_token {
            return Ok(PollOutcome::Authorized(AccessToken(token)));
        }
        match response.error.as_deref() {
            Some("authorization_pending") => Ok(PollOutcome::Pending),
            Some("slow_down") => Ok(PollOutcome::SlowDown),
            Some("access_denied") => Ok(PollOutcome::Denied),
            Some("expired_token") => Ok(PollOutcome::Expired),
            Some(error) => Err(DeviceFlowError::Api(
                response
                    .error_description
                    .unwrap_or_else(|| error.to_owned()),
            )),
            None => Err(DeviceFlowError::Api(
                "response contained neither a token nor an error".to_owned(),
            )),
        }
    }
}

#[derive(Debug)]
pub struct DeviceAuthorization {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in_seconds: u64,
    interval_seconds: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PollOutcome {
    Pending,
    SlowDown,
    Authorized(AccessToken),
    Denied,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DeviceFlowStatus {
    Idle,
    Pending {
        user_code: String,
        verification_uri: String,
        expires_in_seconds: u64,
        interval_seconds: u64,
    },
    SlowDown {
        user_code: String,
        verification_uri: String,
        expires_in_seconds: u64,
        interval_seconds: u64,
    },
    Authorized,
    Denied,
    Expired,
    Cancelled,
    Failed {
        message: String,
    },
}

impl DeviceFlowStatus {
    fn pending(authorization: &DeviceAuthorization) -> Self {
        Self::Pending {
            user_code: authorization.user_code.clone(),
            verification_uri: authorization.verification_uri.clone(),
            expires_in_seconds: authorization.expires_in_seconds,
            interval_seconds: authorization.interval_seconds,
        }
    }

    fn slow_down(authorization: &DeviceAuthorization, interval_seconds: u64) -> Self {
        Self::SlowDown {
            user_code: authorization.user_code.clone(),
            verification_uri: authorization.verification_uri.clone(),
            expires_in_seconds: authorization.expires_in_seconds,
            interval_seconds,
        }
    }
}

#[derive(Clone)]
pub struct DeviceFlowController {
    client: DeviceFlowClient,
    state: Arc<Mutex<ControllerState>>,
}

struct ControllerState {
    status: DeviceFlowStatus,
    token: Option<AccessToken>,
    polling: Option<AbortHandle>,
    generation: u64,
}

impl DeviceFlowController {
    pub fn new(client: DeviceFlowClient) -> Self {
        Self {
            client,
            state: Arc::new(Mutex::new(ControllerState {
                status: DeviceFlowStatus::Idle,
                token: None,
                polling: None,
                generation: 0,
            })),
        }
    }

    pub async fn begin(&self) -> Result<DeviceFlowStatus, DeviceFlowError> {
        let generation = {
            let mut state = self.state.lock().await;
            if let Some(polling) = state.polling.take() {
                polling.abort();
            }
            state.generation = state.generation.wrapping_add(1);
            state.token = None;
            state.status = DeviceFlowStatus::Idle;
            state.generation
        };

        let authorization = match self.client.request_code().await {
            Ok(authorization) => authorization,
            Err(error) => {
                self.set_status_if_current(
                    generation,
                    DeviceFlowStatus::Failed {
                        message: error.to_string(),
                    },
                )
                .await;
                return Err(error);
            }
        };
        let status = DeviceFlowStatus::pending(&authorization);
        self.set_status_if_current(generation, status.clone()).await;

        let controller = self.clone();
        let task = tokio::spawn(async move {
            controller
                .poll_until_terminal(generation, authorization)
                .await;
        });
        let abort_handle = task.abort_handle();
        drop(task);
        let mut state = self.state.lock().await;
        if state.generation == generation {
            state.polling = Some(abort_handle);
        } else {
            abort_handle.abort();
        }
        Ok(status)
    }

    pub async fn status(&self) -> DeviceFlowStatus {
        self.state.lock().await.status.clone()
    }

    pub async fn cancel(&self) {
        let mut state = self.state.lock().await;
        state.generation = state.generation.wrapping_add(1);
        if let Some(polling) = state.polling.take() {
            polling.abort();
        }
        state.token = None;
        state.status = DeviceFlowStatus::Cancelled;
    }

    /// Removes an authorized token from native controller memory.
    ///
    /// This is intentionally not serializable and is never part of a command response.
    pub async fn take_token(&self) -> Option<AccessToken> {
        self.state.lock().await.token.take()
    }

    async fn poll_until_terminal(&self, generation: u64, authorization: DeviceAuthorization) {
        let deadline = Instant::now() + Duration::from_secs(authorization.expires_in_seconds);
        let mut interval_seconds = authorization.interval_seconds;
        loop {
            sleep(Duration::from_secs(interval_seconds)).await;
            if Instant::now() >= deadline {
                self.set_status_if_current(generation, DeviceFlowStatus::Expired)
                    .await;
                return;
            }
            match self.client.poll_once(&authorization).await {
                Ok(PollOutcome::Pending) => {}
                Ok(PollOutcome::SlowDown) => {
                    interval_seconds = interval_seconds.saturating_add(5);
                    self.set_status_if_current(
                        generation,
                        DeviceFlowStatus::slow_down(&authorization, interval_seconds),
                    )
                    .await;
                }
                Ok(PollOutcome::Authorized(token)) => {
                    let mut state = self.state.lock().await;
                    if state.generation == generation {
                        state.token = Some(token);
                        state.status = DeviceFlowStatus::Authorized;
                        state.polling = None;
                    }
                    return;
                }
                Ok(PollOutcome::Denied) => {
                    self.set_status_if_current(generation, DeviceFlowStatus::Denied)
                        .await;
                    return;
                }
                Ok(PollOutcome::Expired) => {
                    self.set_status_if_current(generation, DeviceFlowStatus::Expired)
                        .await;
                    return;
                }
                Err(error) => {
                    self.set_status_if_current(
                        generation,
                        DeviceFlowStatus::Failed {
                            message: error.to_string(),
                        },
                    )
                    .await;
                    return;
                }
            }
        }
    }

    async fn set_status_if_current(&self, generation: u64, status: DeviceFlowStatus) {
        let mut state = self.state.lock().await;
        if state.generation == generation {
            state.status = status;
        }
    }
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}
