use std::future::IntoFuture;
use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::header::{HeaderName, HeaderValue};
use axum::http::{Request, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::ParseError;
use tracing_subscriber::fmt;
use tracing_subscriber::util::{SubscriberInitExt, TryInitError};
use wikinext_app::config::{LogFormat, ObservabilityConfig};
use wikinext_app::{AppServices, DiagnosticReport};

const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");
const CONTENT_SECURITY_POLICY: HeaderName = HeaderName::from_static("content-security-policy");
const PERMISSIONS_POLICY: HeaderName = HeaderName::from_static("permissions-policy");

#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("некорректный фильтр логов")]
    Filter(#[from] ParseError),
    #[error("логирование уже инициализировано")]
    AlreadyInitialized(#[from] TryInitError),
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("не удалось открыть HTTP-сокет")]
    Bind(#[source] std::io::Error),
    #[error("HTTP-сервер завершился с ошибкой")]
    Serve(#[source] std::io::Error),
    #[error("graceful shutdown превысил лимит {0:?}")]
    ShutdownTimeout(Duration),
}

#[derive(Clone)]
struct HttpState {
    services: AppServices,
}

#[derive(Debug, Serialize)]
struct Liveness {
    status: &'static str,
}

pub fn init_tracing(config: &ObservabilityConfig) -> Result<(), TelemetryError> {
    let filter = EnvFilter::try_new(&config.filter)?;

    match config.format {
        LogFormat::Pretty => fmt().with_env_filter(filter).finish().try_init()?,
        LogFormat::Json => fmt()
            .json()
            .with_current_span(true)
            .with_span_list(true)
            .with_env_filter(filter)
            .finish()
            .try_init()?,
    }

    Ok(())
}

pub fn router(services: AppServices, request_timeout: Duration) -> Router {
    let status_routes = Router::new()
        .route("/readyz", get(readiness))
        .route("/status/search", get(search_status))
        .with_state(HttpState { services });

    apply_http_layers(liveness_router().merge(status_routes), request_timeout)
}

pub async fn serve(
    bind: std::net::SocketAddr,
    services: AppServices,
    request_timeout: Duration,
    shutdown_timeout: Duration,
) -> Result<(), ServerError> {
    let listener = TcpListener::bind(bind).await.map_err(ServerError::Bind)?;
    let local_address = listener.local_addr().map_err(ServerError::Bind)?;
    info!(address = %local_address, "HTTP-сервер запущен");

    let (shutdown_sender, shutdown_receiver) = watch::channel(false);
    let deadline_receiver = shutdown_sender.subscribe();
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        let _ = shutdown_sender.send(true);
    });

    let graceful_signal = wait_for_notification(shutdown_receiver);
    let server = axum::serve(listener, router(services, request_timeout))
        .with_graceful_shutdown(graceful_signal)
        .into_future();
    tokio::pin!(server);

    let shutdown_deadline = async move {
        wait_for_notification(deadline_receiver).await;
        tokio::time::sleep(shutdown_timeout).await;
    };
    tokio::pin!(shutdown_deadline);

    tokio::select! {
        result = &mut server => result.map_err(ServerError::Serve),
        () = &mut shutdown_deadline => {
            warn!(?shutdown_timeout, "принудительное завершение после истечения deadline");
            Err(ServerError::ShutdownTimeout(shutdown_timeout))
        }
    }
}

fn liveness_router() -> Router {
    Router::new().route("/healthz", get(liveness))
}

fn apply_http_layers(router: Router, request_timeout: Duration) -> Router {
    let layers = ServiceBuilder::new()
        .layer(middleware::from_fn(discard_client_request_id))
        .layer(SetRequestIdLayer::new(
            REQUEST_ID_HEADER.clone(),
            MakeRequestUuid,
        ))
        .layer(PropagateRequestIdLayer::new(REQUEST_ID_HEADER.clone()))
        // Response header layers stay outside timeout and panic handling so
        // generated error responses receive the same security policy.
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'",
            ),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            PERMISSIONS_POLICY,
            HeaderValue::from_static(
                "accelerometer=(), camera=(), geolocation=(), microphone=(), payment=()",
            ),
        ))
        .layer(TraceLayer::new_for_http().make_span_with(make_http_span))
        .layer(CatchPanicLayer::new())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            request_timeout,
        ));

    router.layer(layers)
}

async fn discard_client_request_id(mut request: Request<Body>, next: Next) -> Response {
    request.headers_mut().remove(&REQUEST_ID_HEADER);
    next.run(request).await
}

fn make_http_span(request: &Request<Body>) -> tracing::Span {
    let request_id = request
        .headers()
        .get(&REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<invalid>");

    tracing::info_span!(
        "http.request",
        request_id,
        method = %request.method(),
        path = %request.uri().path(),
        version = ?request.version(),
    )
}

async fn liveness() -> (StatusCode, Json<Liveness>) {
    (StatusCode::OK, Json(Liveness { status: "ok" }))
}

async fn readiness(State(state): State<HttpState>) -> (StatusCode, Json<DiagnosticReport>) {
    report_response(state.services.readiness().await)
}

async fn search_status(State(state): State<HttpState>) -> (StatusCode, Json<DiagnosticReport>) {
    report_response(state.services.search_status().await)
}

fn report_response(report: DiagnosticReport) -> (StatusCode, Json<DiagnosticReport>) {
    let status = if report.healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(report))
}

async fn wait_for_notification(mut receiver: watch::Receiver<bool>) {
    if *receiver.borrow() {
        return;
    }
    let _ = receiver.wait_for(|notified| *notified).await;
}

async fn wait_for_shutdown_signal() {
    let interrupt = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            warn!(%error, "не удалось установить обработчик Ctrl+C");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                warn!(%error, "не удалось установить обработчик SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }

    info!("получен сигнал завершения");
}

#[cfg(test)]
mod tests {
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn liveness_is_dependency_independent_and_hardened() {
        let response = apply_http_layers(liveness_router(), Duration::from_secs(1))
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("request succeeds");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::X_CONTENT_TYPE_OPTIONS),
            Some(&HeaderValue::from_static("nosniff"))
        );
        assert!(response.headers().contains_key(REQUEST_ID_HEADER));
    }

    #[tokio::test]
    async fn replaces_untrusted_client_request_id() {
        let response = apply_http_layers(liveness_router(), Duration::from_secs(1))
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .header(REQUEST_ID_HEADER.clone(), "attacker-controlled")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("request succeeds");

        let request_id = response
            .headers()
            .get(&REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .expect("server request ID is present");
        assert_ne!(request_id, "attacker-controlled");
        assert_eq!(request_id.len(), 36);
    }

    #[tokio::test]
    async fn timeout_response_keeps_request_id_and_security_headers() {
        let slow = Router::new().route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                StatusCode::OK
            }),
        );
        let response = apply_http_layers(slow, Duration::from_millis(1))
            .oneshot(
                Request::builder()
                    .uri("/slow")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("request succeeds");

        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(
            response.headers().get(header::X_CONTENT_TYPE_OPTIONS),
            Some(&HeaderValue::from_static("nosniff"))
        );
        assert!(response.headers().contains_key(CONTENT_SECURITY_POLICY));
        assert!(response.headers().contains_key(REQUEST_ID_HEADER));
    }

    #[tokio::test]
    async fn panic_response_keeps_request_id_and_security_headers() {
        async fn panic_handler() -> StatusCode {
            panic!("intentional middleware test panic");
        }

        let panicking = Router::new().route("/panic", get(panic_handler));
        let response = apply_http_layers(panicking, Duration::from_secs(1))
            .oneshot(
                Request::builder()
                    .uri("/panic")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("panic is converted to a response");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response.headers().get(header::X_CONTENT_TYPE_OPTIONS),
            Some(&HeaderValue::from_static("nosniff"))
        );
        assert!(response.headers().contains_key(CONTENT_SECURITY_POLICY));
        assert!(response.headers().contains_key(REQUEST_ID_HEADER));
    }

    #[test]
    fn unhealthy_report_maps_to_service_unavailable() {
        let report = DiagnosticReport {
            healthy: false,
            checks: Vec::new(),
        };

        assert_eq!(report_response(report).0, StatusCode::SERVICE_UNAVAILABLE);
    }
}
