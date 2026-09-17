//! Filing adapters. Live submission is capability-gated.

use fidryn_core::Value;
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AdapterResult {
    DryRun {
        packet: Value,
    },
    /// Transport receipt. Does not establish a `Filed` proposition.
    Submitted {
        filing_id: String,
        raw: String,
    },
    Rejected {
        reason: String,
    },
}

pub trait FilingAdapter {
    fn name(&self) -> &'static str;
    fn submit(&self, packet: &Value, live: bool) -> Result<AdapterResult, AdapterError>;
}

pub fn live_allowed() -> bool {
    std::env::var("FIDRYN_ALLOW_LIVE_FILING").ok().as_deref() == Some("1")
}

/// HTTP POST used by [`MassachusettsCorporations`] for live filing.
pub type HttpPost = fn(&str, &str) -> Result<String, String>;

pub struct DryRun;

impl FilingAdapter for DryRun {
    fn name(&self) -> &'static str {
        "dry-run"
    }

    fn submit(&self, packet: &Value, live: bool) -> Result<AdapterResult, AdapterError> {
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
/// Live posts only when `live && live_allowed()`. The endpoint and HTTP
/// client are injected so tests can bind a local mock. Transmission is not
/// a `Filed` fact.
pub struct MassachusettsCorporations {
    pub endpoint: String,
    /// `Ok(body)` is a 2xx response; `Err` is transport or `HTTP <code>: ...`.
    pub post: HttpPost,
}

impl MassachusettsCorporations {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            post: default_post,
        }
    }
}

impl FilingAdapter for MassachusettsCorporations {
    fn name(&self) -> &'static str {
        "ma-corporations"
    }

    fn submit(&self, packet: &Value, live: bool) -> Result<AdapterResult, AdapterError> {
        if !live {
            return DryRun.submit(packet, false);
        }
        if !live_allowed() {
            return Err(AdapterError::LiveDisabled);
        }
        let body = serde_json::to_string(packet)
            .map_err(|e| AdapterError::Transport(format!("serialize packet: {e}")))?;
        outcome_from_post((self.post)(&self.endpoint, &body))
    }
}

/// Default live client: HTTP POST of `body` to `url` via ureq.
pub fn default_post(url: &str, body: &str) -> Result<String, String> {
    match ureq::post(url)
        .timeout(Duration::from_secs(5))
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .send_string(body)
    {
        Ok(resp) => resp.into_string().map_err(|e| e.to_string()),
        Err(ureq::Error::Status(code, resp)) => {
            let text = resp.into_string().unwrap_or_default();
            Err(format!("HTTP {code}: {text}"))
        }
        Err(err) => Err(err.to_string()),
    }
}

fn outcome_from_post(result: Result<String, String>) -> Result<AdapterResult, AdapterError> {
    match result {
        Ok(raw) => Ok(AdapterResult::Submitted {
            filing_id: filing_id_from_body(&raw),
            raw,
        }),
        Err(msg) if is_http_rejection(&msg) => Ok(AdapterResult::Rejected { reason: msg }),
        Err(msg) => Err(AdapterError::Transport(msg)),
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

fn is_http_rejection(msg: &str) -> bool {
    let Some(rest) = msg.strip_prefix("HTTP ") else {
        return false;
    };
    let code = rest
        .get(..3)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    (400..600).contains(&code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpListener};
    use std::sync::{Mutex, MutexGuard};
    use std::time::Duration;

    static LIVE_ENV: Mutex<()> = Mutex::new(());

    struct LiveFilingEnv {
        _guard: MutexGuard<'static, ()>,
        previous: Option<String>,
    }

    impl LiveFilingEnv {
        fn apply(value: Option<&str>) -> Self {
            let guard = LIVE_ENV.lock().unwrap_or_else(|e| e.into_inner());
            let previous = std::env::var("FIDRYN_ALLOW_LIVE_FILING").ok();
            // SAFETY: `LIVE_ENV` serializes mutation of this variable in-crate.
            unsafe {
                match value {
                    Some(v) => std::env::set_var("FIDRYN_ALLOW_LIVE_FILING", v),
                    None => std::env::remove_var("FIDRYN_ALLOW_LIVE_FILING"),
                }
            }
            Self {
                _guard: guard,
                previous,
            }
        }

        fn allow() -> Self {
            Self::apply(Some("1"))
        }

        fn deny() -> Self {
            Self::apply(None)
        }
    }

    impl Drop for LiveFilingEnv {
        fn drop(&mut self) {
            // SAFETY: same lock as `apply`; restores the prior value.
            unsafe {
                match &self.previous {
                    Some(v) => std::env::set_var("FIDRYN_ALLOW_LIVE_FILING", v),
                    None => std::env::remove_var("FIDRYN_ALLOW_LIVE_FILING"),
                }
            }
        }
    }

    fn spawn_json_server(status: &str, body: &str) -> (String, std::thread::JoinHandle<()>) {
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
                        let _ = stream.read(&mut buf);
                        let resp = format!(
                            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(resp.as_bytes());
                        let _ = stream.flush();
                        let _ = stream.shutdown(Shutdown::Both);
                        break;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() >= deadline {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        (format!("http://{addr}/filing"), handle)
    }

    fn reject_post(_url: &str, _body: &str) -> Result<String, String> {
        Err("HTTP 400: malformed certificate".into())
    }

    fn fail_post(_url: &str, _body: &str) -> Result<String, String> {
        Err("connection refused".into())
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
        let _env = LiveFilingEnv::deny();
        let ma = MassachusettsCorporations::new("http://127.0.0.1:9/filing");
        let err = ma.submit(&Value::Unit, true).unwrap_err();
        assert_eq!(err, AdapterError::LiveDisabled);
    }

    #[test]
    fn live_http_submit_returns_submitted_not_filed() {
        let _env = LiveFilingEnv::allow();
        let (url, server) = spawn_json_server("200 OK", r#"{"filing_id":"F-1"}"#);
        let ma = MassachusettsCorporations {
            endpoint: url,
            post: default_post,
        };
        let out = ma
            .submit(&Value::String("certificate".into()), true)
            .expect("live post");
        let _ = server.join();
        match out {
            AdapterResult::Submitted { filing_id, raw } => {
                assert_eq!(filing_id, "F-1");
                assert!(raw.contains("F-1"));
            }
            other => panic!("transmission must stay a receipt, not Filed: {other:?}"),
        }
    }

    #[test]
    fn live_http_client_rejection_is_rejected() {
        let _env = LiveFilingEnv::allow();
        let ma = MassachusettsCorporations {
            endpoint: "http://127.0.0.1:9/filing".into(),
            post: reject_post,
        };
        let out = ma.submit(&Value::Unit, true).unwrap();
        assert!(matches!(out, AdapterResult::Rejected { .. }));
    }

    #[test]
    fn live_http_transport_error() {
        let _env = LiveFilingEnv::allow();
        let ma = MassachusettsCorporations {
            endpoint: "http://127.0.0.1:9/filing".into(),
            post: fail_post,
        };
        let err = ma.submit(&Value::Unit, true).unwrap_err();
        assert!(matches!(err, AdapterError::Transport(_)));
    }

    #[test]
    fn missing_filing_id_is_unknown() {
        assert_eq!(filing_id_from_body("not-json"), "unknown");
        assert_eq!(filing_id_from_body("{}"), "unknown");
        assert_eq!(filing_id_from_body(r#"{"filingId":"Z"}"#), "Z");
    }
}
