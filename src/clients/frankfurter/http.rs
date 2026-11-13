use crate::clients::frankfurter::config::FrankfurterConfig;
use crate::clients::frankfurter::error::FrankfurterError;
use crate::clients::response::{read_limited, ResponseLimitError};
use reqwest::Client;
use serde::de::DeserializeOwned;
/// GET-oriented HTTP client for the Frankfurter v2 API.
#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    base_url: String,
    max_response_bytes: usize,
}
impl HttpClient {
    pub fn new(base_url: String, config: &FrankfurterConfig) -> Self {
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
            base_url,
            max_response_bytes: config.max_response_bytes.max(1),
        }
    }
    fn url(&self, path: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        if path.starts_with('/') {
            format!("{}{}", base, path)
        } else {
            format!("{}/{}", base, path)
        }
    }
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, FrankfurterError> {
        let request = self.client.get(self.url(path));
        self.request_json(request).await
    }
    /// GET with URL-encoded query parameter pairs; a pair with an empty or
    /// unset value simply must not be included by the caller.
    pub async fn get_query<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&'static str, String)],
    ) -> Result<T, FrankfurterError> {
        let request = self.client.get(self.url(path)).query(params);
        self.request_json(request).await
    }
    pub async fn get_text(&self, path: &str) -> Result<String, FrankfurterError> {
        let response = self
            .client
            .get(self.url(path))
            .send()
            .await?
            .error_for_status()?;
        let bytes = read_limited(response, self.max_response_bytes)
            .await
            .map_err(limit_error)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
    async fn request_json<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, FrankfurterError> {
        let response = request.send().await?.error_for_status()?;
        let bytes = read_limited(response, self.max_response_bytes)
            .await
            .map_err(limit_error)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

fn limit_error(error: ResponseLimitError) -> FrankfurterError {
    match error {
        ResponseLimitError::TooLarge => FrankfurterError::ResponseTooLarge,
        ResponseLimitError::Request(error) => FrankfurterError::Http(error),
    }
}
