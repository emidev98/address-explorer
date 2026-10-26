use crate::clients::coinmetrics::config::CoinMetricsConfig;
use crate::clients::coinmetrics::error::CoinMetricsError;
use crate::clients::coinmetrics::types::ErrorResponse;
use reqwest::{Client, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// HTTP client for the Coin Metrics v4 API. Adds the configured `api_key` to
/// every request and translates API error responses into
/// [`CoinMetricsError::Api`].
#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl HttpClient {
    pub fn new(config: &CoinMetricsConfig) -> Self {
        let mut builder = Client::builder();
        if let Some(headers) = &config.headers {
            builder = builder.default_headers(headers.clone());
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(timeout) = config.timeout {
            builder = builder.timeout(timeout);
        }
        let client = builder.build().unwrap_or_else(|_| Client::new());
        Self {
            client,
            base_url: config.base_url(),
            api_key: config.api_key.clone(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path.trim_start_matches('/'))
    }

    fn with_api_key(&self, request: RequestBuilder) -> RequestBuilder {
        match &self.api_key {
            Some(api_key) => request.query(&[("api_key", api_key)]),
            None => request,
        }
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, CoinMetricsError> {
        let request = self.with_api_key(self.client.get(self.url(path)));
        self.send_json(request).await
    }

    pub async fn get_query<T, Q>(&self, path: &str, query: &Q) -> Result<T, CoinMetricsError>
    where
        T: DeserializeOwned,
        Q: Serialize + ?Sized,
    {
        let request = self.with_api_key(self.client.get(self.url(path)).query(query));
        self.send_json(request).await
    }

    pub async fn put_json<T, B>(&self, path: &str, body: &B) -> Result<T, CoinMetricsError>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        let request = self.with_api_key(self.client.put(self.url(path)).json(body));
        self.send_json(request).await
    }

    /// Builds an authenticated WebSocket URL for one of the
    /// `/timeseries-stream/*` endpoints without opening a connection.
    pub fn stream_url<Q>(&self, path: &str, query: &Q) -> Result<String, CoinMetricsError>
    where
        Q: Serialize + ?Sized,
    {
        let request = self
            .with_api_key(self.client.get(self.url(path)).query(query))
            .build()?;
        let mut url = request.url().clone();
        let scheme = if url.scheme() == "http" { "ws" } else { "wss" };
        url.set_scheme(scheme)
            .map_err(|_| CoinMetricsError::Url(url.to_string()))?;
        Ok(url.to_string())
    }

    async fn send_json<T: DeserializeOwned>(
        &self,
        request: RequestBuilder,
    ) -> Result<T, CoinMetricsError> {
        let response = request.send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(api_error(status.as_u16(), &body));
        }
        Ok(response.json::<T>().await?)
    }
}

fn api_error(status: u16, body: &str) -> CoinMetricsError {
    match serde_json::from_str::<ErrorResponse>(body) {
        Ok(response) => CoinMetricsError::Api {
            status,
            error_type: response.error.r#type,
            message: response.error.message,
        },
        Err(_) => CoinMetricsError::Api {
            status,
            error_type: "http".to_string(),
            message: body.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_api_error_bodies() {
        let error = api_error(
            403,
            r#"{"error":{"type":"forbidden","message":"Requested metric is not available."}}"#,
        );
        match error {
            CoinMetricsError::Api {
                status,
                error_type,
                message,
            } => {
                assert_eq!(status, 403);
                assert_eq!(error_type, "forbidden");
                assert_eq!(message, "Requested metric is not available.");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn falls_back_to_the_raw_body_for_non_json_errors() {
        let error = api_error(502, "bad gateway");
        assert_eq!(
            error.to_string(),
            "coin metrics api error (502 http): bad gateway"
        );
    }
}
