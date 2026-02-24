use reqwest::header::{
    HeaderName, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, COOKIE, LOCATION, TRANSFER_ENCODING,
};
use reqwest::{Method, Request, RequestBuilder, Response, StatusCode, Url};

const DEFAULT_MAX_REDIRECTS: usize = 5;

#[derive(Debug, Clone)]
pub struct SafeHttpClient {
    client: reqwest::Client,
    max_redirects: usize,
}

impl SafeHttpClient {
    pub fn new() -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            client,
            max_redirects: DEFAULT_MAX_REDIRECTS,
        })
    }

    pub fn request(&self, method: Method, url: &str) -> Result<RequestBuilder, SafeHttpError> {
        let parsed = Url::parse(url).map_err(|err| SafeHttpError::InvalidUrl(err.to_string()))?;
        validate_scheme(&parsed)?;
        Ok(self.client.request(method, parsed))
    }

    pub async fn send(&self, builder: RequestBuilder) -> Result<Response, SafeHttpError> {
        let request = builder.build().map_err(SafeHttpError::RequestBuild)?;
        self.execute(request).await
    }

    pub async fn execute(&self, request: Request) -> Result<Response, SafeHttpError> {
        let mut current = request;

        for redirect_count in 0..=self.max_redirects {
            validate_scheme(current.url())?;
            let Some(redirect_basis) = current.try_clone() else {
                return Err(SafeHttpError::NonReplayableRequest);
            };
            let response = self
                .client
                .execute(current)
                .await
                .map_err(SafeHttpError::Network)?;

            if !should_follow_redirect(response.status()) {
                return Ok(response);
            }

            if redirect_count == self.max_redirects {
                return Err(SafeHttpError::RedirectLimitExceeded(self.max_redirects));
            }

            let location = response
                .headers()
                .get(LOCATION)
                .ok_or(SafeHttpError::MissingRedirectLocation)?
                .to_str()
                .map_err(SafeHttpError::InvalidRedirectLocation)?;
            let next_url = response
                .url()
                .join(location)
                .map_err(|err| SafeHttpError::InvalidRedirectUrl(err.to_string()))?;
            validate_scheme(&next_url)?;

            let host_changed = authority(redirect_basis.url()) != authority(&next_url);
            current = redirected_request(redirect_basis, response.status(), next_url, host_changed);
        }

        Err(SafeHttpError::RedirectLimitExceeded(self.max_redirects))
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SafeHttpError {
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("invalid redirect URL: {0}")]
    InvalidRedirectUrl(String),
    #[error("invalid redirect location header: {0}")]
    InvalidRedirectLocation(reqwest::header::ToStrError),
    #[error("request build error: {0}")]
    RequestBuild(reqwest::Error),
    #[error("network error: {0}")]
    Network(reqwest::Error),
    #[error("unsupported URL scheme '{0}'")]
    UnsupportedScheme(String),
    #[error("redirect limit exceeded ({0})")]
    RedirectLimitExceeded(usize),
    #[error("redirect response missing Location header")]
    MissingRedirectLocation,
    #[error("request cannot be replayed across redirects")]
    NonReplayableRequest,
}

fn validate_scheme(url: &Url) -> Result<(), SafeHttpError> {
    match url.scheme() {
        "http" | "https" => Ok(()),
        other => Err(SafeHttpError::UnsupportedScheme(other.to_string())),
    }
}

fn should_follow_redirect(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::SEE_OTHER
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT
    )
}

fn redirected_request(
    mut request: Request,
    status: StatusCode,
    next_url: Url,
    host_changed: bool,
) -> Request {
    let original_method = request.method().clone();
    let redirect_method = redirected_method(&original_method, status);

    *request.url_mut() = next_url;
    *request.method_mut() = redirect_method.clone();

    if redirect_method == Method::GET || redirect_method == Method::HEAD {
        *request.body_mut() = None;
        request.headers_mut().remove(CONTENT_LENGTH);
        request.headers_mut().remove(CONTENT_TYPE);
        request.headers_mut().remove(TRANSFER_ENCODING);
    }

    if host_changed {
        strip_sensitive_headers(request.headers_mut());
    }

    request
}

fn redirected_method(original: &Method, status: StatusCode) -> Method {
    match status {
        StatusCode::SEE_OTHER => Method::GET,
        StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND => {
            if original == Method::GET || original == Method::HEAD {
                original.clone()
            } else {
                Method::GET
            }
        }
        StatusCode::TEMPORARY_REDIRECT | StatusCode::PERMANENT_REDIRECT => original.clone(),
        _ => original.clone(),
    }
}

fn strip_sensitive_headers(headers: &mut reqwest::header::HeaderMap) {
    let to_remove = headers
        .keys()
        .filter(|name| is_sensitive_header(name))
        .cloned()
        .collect::<Vec<_>>();
    for name in to_remove {
        headers.remove(name);
    }
}

fn is_sensitive_header(name: &HeaderName) -> bool {
    if name == AUTHORIZATION || name == COOKIE {
        return true;
    }

    let value = name.as_str().to_ascii_lowercase();
    matches!(
        value.as_str(),
        "x-loopflow-token" | "x-loopflow-session-token" | "x-lfd-token" | "x-session-token"
    ) || (value.starts_with("x-") && value.contains("token"))
}

fn authority(url: &Url) -> Option<(String, Option<u16>)> {
    let host = url.host_str()?.to_ascii_lowercase();
    Some((host, url.port_or_known_default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use axum::extract::State;
    use axum::routing::get;
    use axum::{http::HeaderMap, Router};
    use reqwest::header::HeaderValue;
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn strips_sensitive_headers_on_cross_host_redirect() {
        let captured = Arc::new(Mutex::new(None::<HeaderMap>));
        let captured_state = captured.clone();
        let target_app = Router::new().route(
            "/target",
            get(move |headers: HeaderMap, State(captured): State<Arc<Mutex<Option<HeaderMap>>>>| async move {
                *captured.lock().await = Some(headers);
                "ok"
            }),
        )
        .with_state(captured_state);
        let (target_addr, _target_task) = spawn_test_server(target_app).await;

        let redirect_url = format!("http://localhost:{}/target", target_addr.port());
        let source_app = Router::new().route(
            "/source",
            get(move || async move {
                (
                    StatusCode::FOUND,
                    [(
                        LOCATION,
                        HeaderValue::from_str(&redirect_url).expect("location"),
                    )],
                )
            }),
        );
        let (source_addr, _source_task) = spawn_test_server(source_app).await;

        let client = SafeHttpClient::new().expect("safe client");
        let builder = client
            .request(
                Method::GET,
                &format!("http://127.0.0.1:{}/source", source_addr.port()),
            )
            .expect("request")
            .header(AUTHORIZATION, "Bearer top-secret-token")
            .header(COOKIE, "session=secret")
            .header("x-loopflow-session-token", "session-secret")
            .header("x-extra", "safe-header");
        let response = client.send(builder).await.expect("request succeeds");
        assert_eq!(response.status(), StatusCode::OK);

        let headers = captured
            .lock()
            .await
            .clone()
            .expect("captured redirect request headers");
        assert!(!headers.contains_key(AUTHORIZATION));
        assert!(!headers.contains_key(COOKIE));
        assert!(!headers.contains_key("x-loopflow-session-token"));
        assert_eq!(
            headers.get("x-extra").and_then(|value| value.to_str().ok()),
            Some("safe-header")
        );
    }

    #[tokio::test]
    async fn rejects_non_http_schemes() {
        let client = SafeHttpClient::new().expect("safe client");
        let err = client
            .request(Method::GET, "ftp://example.com/resource")
            .expect_err("non-http scheme should fail");
        assert!(matches!(err, SafeHttpError::UnsupportedScheme(_)));
    }

    async fn spawn_test_server(app: Router) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test app");
        });
        (addr, task)
    }
}
