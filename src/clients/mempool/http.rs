use crate::clients::mempool::error::MempoolError;
use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;

#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    base_url: String,
}

impl HttpClient {
    pub fn new(base_url: String, config: &crate::clients::mempool::config::MempoolConfig) -> Self {
        let mut builder = Client::builder();
        if let Some(headers) = &config.headers {
            builder = builder.default_headers(headers.clone());
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(timeout) = config.timeout {
            builder = builder.timeout(timeout);
        }
        let client = builder.build().unwrap_or_else(|_| Client::new());
        Self { client, base_url }
    }

    fn url(&self, path: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        if path.starts_with('/') {
            format!("{}{}", base, path)
        } else {
            format!("{}/{}", base, path)
        }
    }

    pub async fn get_text(&self, path: &str) -> Result<String, MempoolError> {
        let response = self.client.get(self.url(path)).send().await?;
        Ok(error_for_status(response)?.text().await?)
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, MempoolError> {
        let response = self.client.get(self.url(path)).send().await?;
        Ok(error_for_status(response)?.json::<T>().await?)
    }

    pub async fn post_text(&self, path: &str, body: String) -> Result<String, MempoolError> {
        let response = self.client.post(self.url(path)).body(body).send().await?;
        Ok(error_for_status(response)?.text().await?)
    }
}

/// `Response::error_for_status`, except that HTTP 429 maps to the dedicated
/// [`MempoolError::RateLimited`] so callers can pause and resume instead of
/// handling it as a generic HTTP failure.
fn error_for_status(response: Response) -> Result<Response, MempoolError> {
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        return Err(MempoolError::RateLimited);
    }
    Ok(response.error_for_status()?)
}
