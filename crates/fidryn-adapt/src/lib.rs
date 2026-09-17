//! Filing adapters. Live submission is capability-gated.

use fidryn_core::{NodeId, Value, canonical_to_vec};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AdapterError {
    #[error("live filing is disabled (set FIDRYN_ALLOW_LIVE_FILING=1 and pass --live)")]
    LiveDisabled,
    #[error("adapter refused: {0}")]
    Refused(String),
    #[error("transport: {0}")]
    Transport(String),
}

/// Operational filing outcomes. HTTP transport never produces a legal `Filed` fact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AdapterResult {
    DryRun {
        packet: Value,
    },
    /// Transport accepted the packet. Does not establish a `Filed` proposition.
    Accepted {
        filing_id: String,
        raw: String,
    },
    /// HTTP 4xx with an authenticated rejection body.
    Rejected {
        reason: String,
    },
    /// Network failure, HTTP 5xx, or timeout before the packet is known to have been sent.
    TransportFailure {
        reason: String,
    },
    /// Timeout after send, or otherwise unknown whether the agency received the packet.
    SubmissionUncertain {
        reason: String,
    },
}

/// Injected live-filing gate. Default is denied.
///
/// Live HTTP still requires the `--live` flag **and** `allowed`. Tests pass this
/// by argument instead of mutating process-global environment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LiveConfig {
    pub allowed: bool,
}

impl LiveConfig {
    pub fn from_env() -> Self {
        Self {
            allowed: std::env::var("FIDRYN_ALLOW_LIVE_FILING").ok().as_deref() == Some("1"),
        }
    }
}

/// Filing packet plus retry key. `idempotency_key` defaults to empty and is then derived.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitPacket {
    pub payload: Value,
    #[serde(default)]
    pub idempotency_key: String,
}

impl SubmitPacket {
    pub fn from_payload(payload: Value) -> Self {
        Self {
            payload,
            idempotency_key: String::new(),
        }
    }

    pub fn effective_key(&self) -> String {
        if self.idempotency_key.is_empty() {
            derive_idempotency_key(&self.payload)
        } else {
            self.idempotency_key.clone()
        }
    }
}

pub trait FilingAdapter {
    fn name(&self) -> &'static str;

    fn submit(&self, packet: &Value, live: bool) -> Result<AdapterResult, AdapterError> {
        self.submit_with(packet, live, LiveConfig::from_env())
    }

    fn submit_with(
        &self,
        packet: &Value,
        live: bool,
        config: LiveConfig,
    ) -> Result<AdapterResult, AdapterError>;
}

pub fn live_allowed() -> bool {
    LiveConfig::from_env().allowed
}

/// Live POST request. `body` is the packet JSON; the key is also sent as `Idempotency-Key`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostBody<'a> {
    pub url: &'a str,
    pub body: &'a str,
    pub idempotency_key: &'a str,
}

/// Classified live-POST failure. Distinct from a legal `Filed` / `Rejected` fact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PostFailure {
    /// HTTP 4xx with a response body from the agency.
    ClientRejection { code: u16, body: String },
    /// HTTP 5xx or other non-4xx error status.
    Server { code: u16, body: String },
    /// I/O deadline. `after_send` is true when the request may have left the client.
    Timeout { after_send: bool, message: String },
    /// DNS, connect, or other network failure before a response.
    Network(String),
}

/// HTTP POST used by [`MassachusettsCorporations`] for live filing.
pub type HttpPost = fn(&PostBody<'_>) -> Result<String, PostFailure>;

pub struct DryRun;

impl FilingAdapter for DryRun {
    fn name(&self) -> &'static str {
        "dry-run"
    }

    fn submit_with(
        &self,
        packet: &Value,
        live: bool,
        _config: LiveConfig,
    ) -> Result<AdapterResult, AdapterError> {
        if live {
            return Err(AdapterError::LiveDisabled);
        }
        Ok(AdapterResult::DryRun {
            packet: packet.clone(),
        })
    }
}

/// Massachusetts Corporations Division adapter.
///
/// Live posts only when `live && config.allowed`. The endpoint and HTTP
/// client are injected so tests can bind a local mock. Transmission is not
/// a `Filed` fact.
pub struct MassachusettsCorporations {
    pub endpoint: String,
    pub post: HttpPost,
}

impl MassachusettsCorporations {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            post: default_post,
        }
    }

    pub fn submit_packet(
        &self,
        packet: &SubmitPacket,
        live: bool,
        config: LiveConfig,
    ) -> Result<AdapterResult, AdapterError> {
        if !live {
            return DryRun.submit_with(&packet.payload, false, config);
        }
        if !config.allowed {
            return Err(AdapterError::LiveDisabled);
        }
        let body = serde_json::to_string(&packet.payload)
            .map_err(|e| AdapterError::Transport(format!("serialize packet: {e}")))?;
        let key = packet.effective_key();
        let request = PostBody {
            url: &self.endpoint,
            body: &body,
            idempotency_key: &key,
        };
        outcome_from_post((self.post)(&request))
    }
}

impl FilingAdapter for MassachusettsCorporations {
    fn name(&self) -> &'static str {
        "ma-corporations"
    }

    fn submit_with(
        &self,
        packet: &Value,
        live: bool,
        config: LiveConfig,
    ) -> Result<AdapterResult, AdapterError> {
        self.submit_packet(&SubmitPacket::from_payload(packet.clone()), live, config)
    }
}

/// Default live client: HTTP POST of `body` to `url` via ureq.
pub fn default_post(request: &PostBody<'_>) -> Result<String, PostFailure> {
    match ureq::post(request.url)
        .timeout(Duration::from_secs(5))
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .set("Idempotency-Key", request.idempotency_key)
        .send_string(request.body)
    {
        Ok(resp) => resp.into_string().map_err(|e| PostFailure::Timeout {
            after_send: true,
            message: e.to_string(),
        }),
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            if (400..500).contains(&code) {
                Err(PostFailure::ClientRejection { code, body })
            } else {
                Err(PostFailure::Server { code, body })
            }
        }
        Err(ureq::Error::Transport(err)) => Err(classify_transport(err)),
    }
}

fn classify_transport(err: ureq::Transport) -> PostFailure {
    let message = err.to_string();
    let timed_out = is_timeout_message(&message);
    match err.kind() {
        ureq::ErrorKind::ConnectionFailed | ureq::ErrorKind::Dns if timed_out => {
            PostFailure::Timeout {
                after_send: false,
                message,
            }
        }
        ureq::ErrorKind::ConnectionFailed | ureq::ErrorKind::Dns => PostFailure::Network(message),
        ureq::ErrorKind::Io if timed_out => PostFailure::Timeout {
            after_send: true,
            message,
        },
        _ if timed_out => PostFailure::Timeout {
            after_send: true,
            message,
        },
        _ => PostFailure::Network(message),
    }
}

fn outcome_from_post(result: Result<String, PostFailure>) -> Result<AdapterResult, AdapterError> {
    match result {
        Ok(raw) => Ok(AdapterResult::Accepted {
            filing_id: filing_id_from_body(&raw),
            raw,
        }),
        Err(PostFailure::ClientRejection { code, body }) => Ok(AdapterResult::Rejected {
            reason: format!("HTTP {code}: {body}"),
        }),
        Err(PostFailure::Server { code, body }) => Ok(AdapterResult::TransportFailure {
            reason: format!("HTTP {code}: {body}"),
        }),
        Err(PostFailure::Timeout {
            after_send: true,
            message,
        }) => Ok(AdapterResult::SubmissionUncertain { reason: message }),
        Err(PostFailure::Timeout {
            after_send: false,
            message,
        })
        | Err(PostFailure::Network(message)) => {
            Ok(AdapterResult::TransportFailure { reason: message })
        }
    }
}

fn filing_id_from_body(raw: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return "unknown".into();
    };
    match value.get("filing_id").or_else(|| value.get("filingId")) {
        Some(serde_json::Value::String(id)) if !id.is_empty() => id.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        _ => "unknown".into(),
    }
}

fn derive_idempotency_key(packet: &Value) -> String {
    let bytes =
        canonical_to_vec(packet).unwrap_or_else(|_| serde_json::to_vec(packet).unwrap_or_default());
    format!("fidryn-{}", NodeId::of(&bytes))
}

fn is_timeout_message(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("timed out") || lower.contains("timeout")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpListener};
    use std::time::Duration;

    fn spawn_json_server(status: &str, body: &str) -> (String, std::thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
        let addr = listener.local_addr().expect("addr");
        let status = status.to_owned();
        let body = body.to_owned();
        let handle = std::thread::spawn(move || {
            listener.set_nonblocking(true).ok();
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
                        let mut buf = [0_u8; 8192];
                        let n = stream.read(&mut buf).unwrap_or(0);
                        let request = buf[..n].to_vec();
                        let resp = format!(
                            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(resp.as_bytes());
                        let _ = stream.flush();
                        let _ = stream.shutdown(Shutdown::Both);
                        return request;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() >= deadline {
                            return Vec::new();
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => return Vec::new(),
                }
            }
        });
        (format!("http://{addr}/filing"), handle)
    }

    fn allow_live() -> LiveConfig {
        LiveConfig { allowed: true }
    }

    fn ma_with(post: HttpPost) -> MassachusettsCorporations {
        MassachusettsCorporations {
            endpoint: "http://127.0.0.1:9/filing".into(),
            post,
        }
    }

    fn reject_post(_request: &PostBody<'_>) -> Result<String, PostFailure> {
        Err(PostFailure::ClientRejection {
            code: 400,
            body: "malformed certificate".into(),
        })
    }

    fn fail_post(_request: &PostBody<'_>) -> Result<String, PostFailure> {
        Err(PostFailure::Network("connection refused".into()))
    }

    fn server_error_post(_request: &PostBody<'_>) -> Result<String, PostFailure> {
        Err(PostFailure::Server {
            code: 500,
            body: "internal error".into(),
        })
    }

    fn timeout_after_send(_request: &PostBody<'_>) -> Result<String, PostFailure> {
        Err(PostFailure::Timeout {
            after_send: true,
            message: "timed out reading response".into(),
        })
    }

    fn timeout_before_send(_request: &PostBody<'_>) -> Result<String, PostFailure> {
        Err(PostFailure::Timeout {
            after_send: false,
            message: "connection timed out".into(),
        })
    }

    #[test]
    fn dry_run_never_files() {
        let out = DryRun
            .submit(&Value::String("packet".into()), false)
            .unwrap();
        assert!(matches!(out, AdapterResult::DryRun { .. }));
    }

    #[test]
    fn massachusetts_not_live_is_dry_run() {
        let ma = MassachusettsCorporations::new("http://127.0.0.1:9/filing");
        let out = ma.submit(&Value::String("packet".into()), false).unwrap();
        assert!(matches!(out, AdapterResult::DryRun { .. }));
    }

    #[test]
    fn live_without_env_is_disabled() {
        let ma = MassachusettsCorporations::new("http://127.0.0.1:9/filing");
        let err = ma
            .submit_with(&Value::Unit, true, LiveConfig::default())
            .unwrap_err();
        assert_eq!(err, AdapterError::LiveDisabled);
    }

    #[test]
    fn live_http_submit_returns_submitted_not_filed() {
        let (url, server) = spawn_json_server("200 OK", r#"{"filing_id":"F-1"}"#);
        let ma = MassachusettsCorporations {
            endpoint: url,
            post: default_post,
        };
        let out = ma
            .submit_with(&Value::String("certificate".into()), true, allow_live())
            .expect("live post");
        let _ = server.join();
        match out {
            AdapterResult::Accepted { filing_id, raw } => {
                assert_eq!(filing_id, "F-1");
                assert!(raw.contains("F-1"));
            }
            other => panic!("transmission must stay a receipt, not Filed: {other:?}"),
        }
    }

    #[test]
    fn live_http_client_rejection_is_rejected() {
        let out = ma_with(reject_post)
            .submit_with(&Value::Unit, true, allow_live())
            .unwrap();
        assert!(matches!(out, AdapterResult::Rejected { .. }));
    }

    #[test]
    fn live_http_transport_error() {
        let out = ma_with(fail_post)
            .submit_with(&Value::Unit, true, allow_live())
            .unwrap();
        assert!(matches!(out, AdapterResult::TransportFailure { .. }));
    }

    #[test]
    fn live_http_5xx_is_transport_failure_not_rejected() {
        let out = ma_with(server_error_post)
            .submit_with(&Value::Unit, true, allow_live())
            .unwrap();
        match out {
            AdapterResult::TransportFailure { reason } => {
                assert!(reason.contains("500"), "{reason}");
            }
            other => panic!("5xx is transport failure, not {other:?}"),
        }
    }

    #[test]
    fn live_http_timeout_after_send_is_uncertain() {
        let out = ma_with(timeout_after_send)
            .submit_with(&Value::Unit, true, allow_live())
            .unwrap();
        assert!(matches!(out, AdapterResult::SubmissionUncertain { .. }));
    }

    #[test]
    fn live_http_timeout_before_send_is_transport_failure() {
        let out = ma_with(timeout_before_send)
            .submit_with(&Value::Unit, true, allow_live())
            .unwrap();
        assert!(matches!(out, AdapterResult::TransportFailure { .. }));
    }

    #[test]
    fn live_http_5xx_from_listener_is_transport_failure() {
        let (url, server) = spawn_json_server("500 Internal Server Error", r#"{"error":"boom"}"#);
        let ma = MassachusettsCorporations {
            endpoint: url,
            post: default_post,
        };
        let out = ma
            .submit_with(&Value::Unit, true, allow_live())
            .expect("classified 5xx");
        let _ = server.join();
        assert!(
            matches!(out, AdapterResult::TransportFailure { .. }),
            "5xx must not be Rejected: {out:?}"
        );
    }

    #[test]
    fn live_http_submit_sends_idempotency_key() {
        let (url, server) = spawn_json_server("200 OK", r#"{"filing_id":"F-1"}"#);
        let ma = MassachusettsCorporations {
            endpoint: url,
            post: default_post,
        };
        let packet = Value::String("certificate".into());
        let expected = SubmitPacket::from_payload(packet.clone()).effective_key();
        let _ = ma
            .submit_with(&packet, true, allow_live())
            .expect("live post");
        let request = server.join().expect("server thread");
        let text = String::from_utf8_lossy(&request);
        assert!(
            text.contains(&format!("Idempotency-Key: {expected}")),
            "missing idempotency header in {text}"
        );
    }

    #[test]
    fn submit_packet_idempotency_key_defaults() {
        let supplied = SubmitPacket {
            payload: Value::Bool(true),
            idempotency_key: "explicit-key".into(),
        };
        assert_eq!(supplied.effective_key(), "explicit-key");

        let derived = SubmitPacket::from_payload(Value::Bool(true));
        assert!(derived.idempotency_key.is_empty());
        let key = derived.effective_key();
        assert!(key.starts_with("fidryn-"));
        assert_eq!(
            SubmitPacket::from_payload(Value::Bool(true)).effective_key(),
            key
        );

        let json = serde_json::to_string(&SubmitPacket::from_payload(Value::Bool(true))).unwrap();
        let parsed: SubmitPacket = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.idempotency_key, "");
    }

    #[test]
    fn missing_filing_id_is_unknown() {
        assert_eq!(filing_id_from_body("not-json"), "unknown");
        assert_eq!(filing_id_from_body("{}"), "unknown");
        assert_eq!(filing_id_from_body(r#"{"filingId":"Z"}"#), "Z");
    }
}
