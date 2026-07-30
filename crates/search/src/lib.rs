use std::fmt;
use std::time::Duration;

use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue};
use reqwest::redirect::Policy;
use reqwest::{Client, StatusCode, Url};
use serde::Deserialize;
use thiserror::Error;
use tracing::instrument;

pub const EXPECTED_MEILISEARCH_VERSION: &str = "1.45.1";
const MAX_DIAGNOSTIC_RESPONSE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchStatus {
    pub version: String,
}

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("некорректный URL Meilisearch")]
    InvalidUrl(#[source] url::ParseError),
    #[error("URL Meilisearch должен использовать http или https")]
    UnsupportedScheme,
    #[error("удалённый Meilisearch требует HTTPS")]
    InsecureRemote,
    #[error("URL Meilisearch не должен содержать логин, пароль, query или fragment")]
    UnsafeUrl,
    #[error("ключ Meilisearch нельзя передать в HTTP-заголовке")]
    InvalidApiKey(#[source] reqwest::header::InvalidHeaderValue),
    #[error("не удалось создать HTTP-клиент Meilisearch")]
    Client(#[source] reqwest::Error),
    #[error("Meilisearch недоступен")]
    Request(#[source] reqwest::Error),
    #[error("Meilisearch вернул HTTP {0}")]
    Http(StatusCode),
    #[error("ответ Meilisearch превышает предел {MAX_DIAGNOSTIC_RESPONSE_BYTES} байт")]
    ResponseTooLarge,
    #[error("Meilisearch вернул некорректный JSON")]
    Decode(#[source] serde_json::Error),
    #[error("Meilisearch сообщил состояние {0:?}")]
    Unhealthy(String),
    #[error("ожидался Meilisearch {expected}, получена версия {actual}")]
    IncompatibleVersion {
        expected: &'static str,
        actual: String,
    },
}

#[derive(Clone)]
pub struct SearchClient {
    base_url: Url,
    http: Client,
}

impl fmt::Debug for SearchClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchClient")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl SearchClient {
    pub fn new(
        base_url: &str,
        api_key: &str,
        request_timeout: Duration,
    ) -> Result<Self, SearchError> {
        let base_url = normalize_base_url(base_url)?;

        let mut authorization = HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(SearchError::InvalidApiKey)?;
        authorization.set_sensitive(true);

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization);
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let http = Client::builder()
            .default_headers(headers)
            // The master key must never leave the direct application-to-Meili
            // connection through a process-wide HTTP(S)_PROXY setting.
            .no_proxy()
            .redirect(Policy::none())
            .timeout(request_timeout)
            .build()
            .map_err(SearchError::Client)?;

        Ok(Self { base_url, http })
    }

    #[instrument(name = "meilisearch.probe", skip(self))]
    pub async fn diagnose(&self) -> Result<SearchStatus, SearchError> {
        let health: HealthResponse = self.get_json("health").await?;
        if health.status != "available" {
            return Err(SearchError::Unhealthy(health.status));
        }

        let version: VersionResponse = self.get_json("version").await?;
        if version.package_version != EXPECTED_MEILISEARCH_VERSION {
            return Err(SearchError::IncompatibleVersion {
                expected: EXPECTED_MEILISEARCH_VERSION,
                actual: version.package_version,
            });
        }

        Ok(SearchStatus {
            version: version.package_version,
        })
    }

    async fn get_json<T>(&self, path: &str) -> Result<T, SearchError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let url = self.base_url.join(path).map_err(SearchError::InvalidUrl)?;
        let mut response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(SearchError::Request)?;

        if !response.status().is_success() {
            return Err(SearchError::Http(response.status()));
        }

        if response
            .content_length()
            .is_some_and(|length| length > MAX_DIAGNOSTIC_RESPONSE_BYTES as u64)
        {
            return Err(SearchError::ResponseTooLarge);
        }

        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(SearchError::Request)? {
            append_bounded(&mut body, &chunk)?;
        }

        serde_json::from_slice(&body).map_err(SearchError::Decode)
    }
}

fn append_bounded(body: &mut Vec<u8>, chunk: &[u8]) -> Result<(), SearchError> {
    let length = body
        .len()
        .checked_add(chunk.len())
        .filter(|length| *length <= MAX_DIAGNOSTIC_RESPONSE_BYTES)
        .ok_or(SearchError::ResponseTooLarge)?;
    body.reserve(length - body.len());
    body.extend_from_slice(chunk);
    Ok(())
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
}

#[derive(Debug, Deserialize)]
struct VersionResponse {
    #[serde(rename = "pkgVersion")]
    package_version: String,
}

fn normalize_base_url(value: &str) -> Result<Url, SearchError> {
    let mut url = Url::parse(value).map_err(SearchError::InvalidUrl)?;

    if !matches!(url.scheme(), "http" | "https") {
        return Err(SearchError::UnsupportedScheme);
    }
    if url.scheme() == "http" && !url_host_is_loopback(&url) {
        return Err(SearchError::InsecureRemote);
    }

    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(SearchError::UnsafeUrl);
    }

    if !url.path().ends_with('/') {
        let mut path = url.path().to_owned();
        path.push('/');
        url.set_path(&path);
    }

    Ok(url)
}

fn url_host_is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(host)) => {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        }
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_base_path() {
        let url = normalize_base_url("http://127.0.0.1:7700").expect("valid URL");
        assert_eq!(url.as_str(), "http://127.0.0.1:7700/");

        let url = normalize_base_url("https://search.example.test/meili").expect("valid URL");
        assert_eq!(url.as_str(), "https://search.example.test/meili/");
    }

    #[test]
    fn rejects_credentials_and_non_http_schemes() {
        assert!(matches!(
            normalize_base_url("ftp://localhost"),
            Err(SearchError::UnsupportedScheme)
        ));
        assert!(matches!(
            normalize_base_url("http://admin:secret@localhost:7700"),
            Err(SearchError::UnsafeUrl)
        ));
    }

    #[test]
    fn plaintext_http_is_loopback_only() {
        for url in [
            "http://localhost:7700",
            "http://127.0.0.1:7700",
            "http://[::1]:7700",
        ] {
            normalize_base_url(url).expect("loopback HTTP is accepted");
        }
        assert!(matches!(
            normalize_base_url("http://search.example.test:7700"),
            Err(SearchError::InsecureRemote)
        ));
        normalize_base_url("https://search.example.test").expect("remote HTTPS is accepted");
    }

    #[test]
    fn client_debug_does_not_expose_api_key() {
        let client = SearchClient::new(
            "http://127.0.0.1:7700",
            "development-master-key",
            Duration::from_secs(1),
        )
        .expect("valid client");

        let rendered = format!("{client:?}");
        assert!(!rendered.contains("development-master-key"));
    }

    #[test]
    fn bounds_diagnostic_response_body() {
        let mut body = vec![0; MAX_DIAGNOSTIC_RESPONSE_BYTES - 1];
        append_bounded(&mut body, &[1]).expect("body exactly at limit is accepted");
        assert!(matches!(
            append_bounded(&mut body, &[2]),
            Err(SearchError::ResponseTooLarge)
        ));
    }
}
