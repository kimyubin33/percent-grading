//! Bounded loopback HTTP adapter for native clients and application backends.
//!
//! This is not an internet deployment configuration. It has no browser CORS,
//! TLS termination, tenant isolation, or durable application job queue. Provider
//! calls may finish after a client disconnects; saved runs prevent silent rerolls.
use crate::{
    VERSION,
    service::{self, Response as ApiResponse, ServiceContext},
};
use http_body_util::{BodyExt, Full, Limited};
use hyper::{
    Method, Request, Response, StatusCode,
    body::{Bytes, Incoming},
    header,
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::{TokioIo, TokioTimer};
use ring::hmac;
use serde_json::{Value, json};
use std::{convert::Infallible, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::Semaphore, time::timeout};

const MAX_CONNECTIONS: usize = 16;
const MAX_JOBS: usize = 4;
const READ_TIMEOUT: Duration = Duration::from_secs(10);

type HttpResponse = Response<Full<Bytes>>;

struct TokenVerifier {
    key: hmac::Key,
    tag: hmac::Tag,
}

impl TokenVerifier {
    fn new(token: &str) -> Result<Self, String> {
        if !(32..=256).contains(&token.len()) || !token.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(
                "API token must contain 32..256 visible ASCII characters without whitespace".into(),
            );
        }
        // The public domain key separates this verifier from unrelated hashes.
        // The token-derived tag stays private; HMAC verification compares tags
        // in constant time without storing the clear token in server state.
        let key = hmac::Key::new(hmac::HMAC_SHA256, b"percent-grading-local-api-token-v1");
        let tag = hmac::sign(&key, token.as_bytes());
        Ok(Self { key, tag })
    }

    fn accepts(&self, candidate: &str) -> bool {
        (32..=256).contains(&candidate.len())
            && hmac::verify(&self.key, candidate.as_bytes(), self.tag.as_ref()).is_ok()
    }
}

struct State {
    context: Arc<ServiceContext>,
    address: SocketAddr,
    verifier: TokenVerifier,
    jobs: Arc<Semaphore>,
}

fn json_response(status: StatusCode, value: &Value) -> HttpResponse {
    // serde_json::Value serialization cannot fail for the values built here.
    let body = serde_json::to_vec(value).expect("JSON response serializes");
    let mut response = Response::new(Full::new(Bytes::from(body)));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/json; charset=utf-8".parse().unwrap(),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert(header::CONNECTION, "close".parse().unwrap());
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    response
}

fn api_response(response: ApiResponse) -> HttpResponse {
    let status =
        StatusCode::from_u16(response.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    json_response(
        status,
        &serde_json::to_value(response).expect("API response serializes"),
    )
}

fn failure(code: &str, message: &str) -> HttpResponse {
    api_response(ApiResponse::failure(None, code, message))
}

fn allowed_host(request: &Request<Incoming>, address: SocketAddr) -> bool {
    if request.headers().get_all(header::HOST).iter().count() != 1 {
        return false;
    }
    request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|value| {
            value.eq_ignore_ascii_case(&address.to_string())
                || value.eq_ignore_ascii_case(&format!("localhost:{}", address.port()))
        })
}

async fn handle(request: Request<Incoming>, state: Arc<State>) -> Result<HttpResponse, Infallible> {
    if !allowed_host(&request, state.address)
        || request.uri().scheme().is_some()
        || request.uri().authority().is_some()
    {
        return Ok(failure(
            "invalid_request",
            "Host must match this loopback server; use an origin-form request target.",
        ));
    }
    // Browser scripts must use their own backend rather than receive host secrets.
    if request.headers().contains_key(header::ORIGIN) {
        return Ok(failure(
            "origin_forbidden",
            "Browser-origin requests are not enabled. Use a native client or your application backend.",
        ));
    }
    if request.uri().query().is_some() {
        return Ok(failure("not_found", "Unknown API route."));
    }
    match request.uri().path() {
        "/health" => {
            if request.method() != Method::GET {
                return Ok(failure("method_not_allowed", "Use GET for this route."));
            }
            return Ok(json_response(
                StatusCode::OK,
                &json!({
                    "status":"ok","api_version":service::API_VERSION,"engine_version":VERSION
                }),
            ));
        }
        "/v1/dispatch" => {
            if request.method() != Method::POST {
                return Ok(failure("method_not_allowed", "Use POST for this route."));
            }
        }
        _ => return Ok(failure("not_found", "Unknown API route.")),
    }
    let authorized = request
        .headers()
        .get_all(header::AUTHORIZATION)
        .iter()
        .count()
        == 1
        && request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .is_some_and(|v| state.verifier.accepts(v));
    if !authorized {
        return Ok(failure(
            "unauthorized",
            "A valid server bearer token is required.",
        ));
    }
    let json_type = request
        .headers()
        .get_all(header::CONTENT_TYPE)
        .iter()
        .count()
        == 1
        && request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    if !json_type {
        return Ok(failure(
            "unsupported_media_type",
            "Use Content-Type: application/json.",
        ));
    }
    if request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .is_some_and(|n| n > service::MAX_REQUEST_BYTES as u64)
    {
        return Ok(failure(
            "payload_too_large",
            "Request exceeds the 2097152-byte API limit.",
        ));
    }
    let permit = match state.jobs.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return Ok(failure(
                "busy",
                "The local server is at its concurrent request limit.",
            ));
        }
    };
    let body = match timeout(
        READ_TIMEOUT,
        Limited::new(request.into_body(), service::MAX_REQUEST_BYTES).collect(),
    )
    .await
    {
        Err(_) => {
            return Ok(failure(
                "request_timeout",
                "Request body did not finish within ten seconds.",
            ));
        }
        Ok(Err(error)) if error.is::<http_body_util::LengthLimitError>() => {
            return Ok(failure(
                "payload_too_large",
                "Request exceeds the 2097152-byte API limit.",
            ));
        }
        Ok(Err(_)) => {
            return Ok(failure("invalid_request", "Request body is invalid."));
        }
        Ok(Ok(collected)) => collected.to_bytes(),
    };
    let context = state.context.clone();
    let result = tokio::task::spawn_blocking(move || {
        // Keep the job permit until blocking grading finishes, even if a client
        // disconnects and drops the async response waiter.
        let _permit = permit;
        service::handle_json(&context, &body)
    })
    .await;
    Ok(match result {
        Ok(response) => api_response(response),
        Err(_) => failure("internal_error", "The request could not be completed."),
    })
}

async fn listen(listener: TcpListener, state: Arc<State>) -> Result<(), String> {
    let connections = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    loop {
        let (stream, _) = listener
            .accept()
            .await
            .map_err(|_| "Could not accept a local API connection")?;
        let Ok(permit) = connections.clone().try_acquire_owned() else {
            // Close excess sockets without accepting unbounded work.
            drop(stream);
            continue;
        };
        let state = state.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let service = service_fn(move |request| handle(request, state.clone()));
            let mut builder = http1::Builder::new();
            builder
                .keep_alive(false)
                .max_buf_size(16 * 1024)
                .timer(TokioTimer::new())
                .header_read_timeout(READ_TIMEOUT);
            let _ = builder
                .serve_connection(TokioIo::new(stream), service)
                .await;
        });
    }
}

/// Start the local HTTP adapter. Explicit host settings own credentials/storage.
/// Provider requests are disabled unless `context.allow_live` is true.
pub fn serve(bind: SocketAddr, context: ServiceContext, token: String) -> Result<(), String> {
    if !bind.ip().is_loopback() {
        return Err("The bundled HTTP adapter binds only to a loopback address".into());
    }
    let verifier = TokenVerifier::new(&token)?;
    drop(token);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|_| "Could not initialize the local API runtime")?;
    runtime.block_on(async move {
        let listener = TcpListener::bind(bind)
            .await
            .map_err(|_| "Could not bind the local API address")?;
        let address = listener
            .local_addr()
            .map_err(|_| "Could not read the local API address")?;
        eprintln!("Percent Grading local API listening at http://{address}");
        let state = Arc::new(State {
            context: Arc::new(context),
            address,
            verifier,
            jobs: Arc::new(Semaphore::new(MAX_JOBS)),
        });
        listen(listener, state).await
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "local-test-token-32-characters-minimum";

    fn with_server<F>(operation: F)
    where
        F: FnOnce(SocketAddr) + Send + 'static,
    {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let state = Arc::new(State {
                context: Arc::new(ServiceContext::default()),
                address,
                verifier: TokenVerifier::new(TOKEN).unwrap(),
                jobs: Arc::new(Semaphore::new(MAX_JOBS)),
            });
            let server = tokio::spawn(listen(listener, state));
            let result = tokio::task::spawn_blocking(move || operation(address)).await;
            server.abort();
            result.unwrap();
        });
    }

    fn client() -> reqwest::blocking::Client {
        let _ = rustls::crypto::ring::default_provider().install_default();
        reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
    }

    async fn memory_request(
        path: &str,
        host: &str,
        token: Option<&str>,
        origin: Option<&str>,
        body: &str,
    ) -> (u16, Value) {
        let (client, server) = tokio::io::duplex(64 * 1024);
        let state = Arc::new(State {
            context: Arc::new(ServiceContext::default()),
            address: "127.0.0.1:8787".parse().unwrap(),
            verifier: TokenVerifier::new(TOKEN).unwrap(),
            jobs: Arc::new(Semaphore::new(MAX_JOBS)),
        });
        let server = tokio::spawn(async move {
            http1::Builder::new()
                .keep_alive(false)
                .serve_connection(
                    TokioIo::new(server),
                    service_fn(move |request| handle(request, state.clone())),
                )
                .await
        });
        let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(client))
            .await
            .unwrap();
        let connection = tokio::spawn(connection);
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(path)
            .header(header::HOST, host)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        if let Some(origin) = origin {
            request = request.header(header::ORIGIN, origin);
        }
        let request = request
            .body(Full::new(Bytes::copy_from_slice(body.as_bytes())))
            .unwrap();
        let response = sender.send_request(request).await.unwrap();
        let status = response.status().as_u16();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let response = serde_json::from_slice(&bytes).unwrap();
        server.await.unwrap().unwrap();
        connection.await.unwrap().unwrap();
        (status, response)
    }

    #[test]
    fn hyper_dispatch_protocol_works_without_network_permissions() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let body =
                json!({"api_version":service::API_VERSION,"request_id":"memory-http","payload":{
                    "operation":"compile","source":include_str!("../policies/apple.aha")
                }})
                .to_string();
            let (status, response) =
                memory_request("/v1/dispatch", "127.0.0.1:8787", Some(TOKEN), None, &body).await;
            assert_eq!(status, 200);
            assert_eq!(response["request_id"], "memory-http");
            assert!(response["result"]["lock"]["contract_hash"].is_string());
            let (status, _) =
                memory_request("/v1/dispatch", "127.0.0.1:8787", None, None, &body).await;
            assert_eq!(status, 401);
            let (status, _) =
                memory_request("/v1/dispatch", "evil.example", Some(TOKEN), None, &body).await;
            assert_eq!(status, 400);
            let (status, _) = memory_request(
                "/v1/dispatch",
                "127.0.0.1:8787",
                Some(TOKEN),
                Some("https://example.org"),
                &body,
            )
            .await;
            assert_eq!(status, 403);
            let (status, response) = memory_request(
                "/v1/dispatch",
                "127.0.0.1:8787",
                Some(TOKEN),
                None,
                "{secret-input",
            )
            .await;
            assert_eq!(status, 400);
            assert_eq!(response["error"]["code"], "invalid_request");
            assert!(!response.to_string().contains("secret-input"));
        });
    }

    #[test]
    fn health_and_authenticated_compile_work_over_actual_http() {
        with_server(|address| {
            let client = client();
            let health: Value = client
                .get(format!("http://{address}/health"))
                .send()
                .unwrap()
                .json()
                .unwrap();
            assert_eq!(health["api_version"], service::API_VERSION);
            let request = json!({"api_version":service::API_VERSION,"request_id":"http-test","payload":{
                "operation":"compile","source":include_str!("../policies/apple.aha")
            }});
            let response = client
                .post(format!("http://{address}/v1/dispatch"))
                .bearer_auth(TOKEN)
                .json(&request)
                .send()
                .unwrap();
            assert_eq!(response.status().as_u16(), 200);
            let response: ApiResponse = response.json().unwrap();
            assert_eq!(response.request_id.as_deref(), Some("http-test"));
            assert!(response.result.unwrap()["lock"]["contract_hash"].is_string());
        });
    }

    #[test]
    fn http_rejects_missing_auth_host_origin_and_oversized_body() {
        with_server(|address| {
            let client = client();
            let url = format!("http://{address}/v1/dispatch");
            assert_eq!(
                client
                    .post(&url)
                    .json(&json!({}))
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                401
            );
            assert_eq!(
                client
                    .post(&url)
                    .bearer_auth(TOKEN)
                    .header(header::HOST, "evil.example")
                    .json(&json!({}))
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                400
            );
            let origin = client
                .post(&url)
                .bearer_auth(TOKEN)
                .header(header::ORIGIN, "https://example.org")
                .json(&json!({}))
                .send()
                .unwrap();
            assert_eq!(origin.status().as_u16(), 403);
            assert!(!origin.headers().contains_key("access-control-allow-origin"));
            assert_eq!(
                client
                    .post(&url)
                    .bearer_auth(TOKEN)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(vec![b' '; service::MAX_REQUEST_BYTES + 1])
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                413
            );
            assert_eq!(
                client
                    .post(&url)
                    .bearer_auth(TOKEN)
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body("{}")
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                415
            );
        });
    }

    #[test]
    fn server_configuration_rejects_public_bind_and_weak_tokens() {
        assert!(
            serve(
                "0.0.0.0:0".parse().unwrap(),
                ServiceContext::default(),
                TOKEN.into()
            )
            .is_err()
        );
        assert!(
            serve(
                "127.0.0.1:0".parse().unwrap(),
                ServiceContext::default(),
                "short".into()
            )
            .is_err()
        );
        assert!(TokenVerifier::new(&" ".repeat(32)).is_err());
        assert!(TokenVerifier::new(&"x".repeat(257)).is_err());
        let verifier = TokenVerifier::new(TOKEN).unwrap();
        assert!(verifier.accepts(TOKEN));
        assert!(!verifier.accepts(&"x".repeat(TOKEN.len())));
    }

    #[test]
    fn http_exposes_structured_parse_errors_without_echoing_input() {
        with_server(|address| {
            let response = client()
                .post(format!("http://{address}/v1/dispatch"))
                .bearer_auth(TOKEN)
                .header(header::CONTENT_TYPE, "application/json")
                .body("{secret-input")
                .send()
                .unwrap();
            assert_eq!(response.status().as_u16(), 400);
            let text = response.text().unwrap();
            assert!(!text.contains("secret-input"));
            let parsed: ApiResponse = serde_json::from_str(&text).unwrap();
            assert!(parsed.request_id.is_none());
            assert_eq!(parsed.error.unwrap().code, "invalid_request");
        });
    }
}
