use crate::clients::mempool::error::MempoolError;
use crate::clients::response::{read_limited, ResponseLimitError};
use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;
#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    base_url: String,
    max_response_bytes: usize,
}
/// Percent-encode one path segment so stored values can never inject `?`,
/// `#`, `/`, or `..` into a third-party URL.
pub fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }
    encoded
}

/// Validate a transaction id before it becomes a query or path segment.
pub fn encode_txid(value: &str) -> Result<String, MempoolError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(MempoolError::InvalidPath);
    }
    Ok(encode_path_segment(value))
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
    pub async fn get_text(&self, path: &str) -> Result<String, MempoolError> {
        let response = self.client.get(self.url(path)).send().await?;
        let response = error_for_status(response)?;
        let bytes = read_limited(response, self.max_response_bytes)
            .await
            .map_err(limit_error)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, MempoolError> {
        let response = self.client.get(self.url(path)).send().await?;
        let response = error_for_status(response)?;
        let bytes = read_limited(response, self.max_response_bytes)
            .await
            .map_err(limit_error)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
    pub async fn post_text(&self, path: &str, body: String) -> Result<String, MempoolError> {
        let response = self.client.post(self.url(path)).body(body).send().await?;
        let response = error_for_status(response)?;
        let bytes = read_limited(response, self.max_response_bytes)
            .await
            .map_err(limit_error)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}
/// `Response::error_for_status`, except that HTTP 429 maps to the dedicated
/// [`MempoolError::RateLimited`] so callers can pause and resume instead of
/// handling it as a generic HTTP failure.
fn limit_error(error: ResponseLimitError) -> MempoolError {
    match error {
        ResponseLimitError::TooLarge => MempoolError::ResponseTooLarge,
        ResponseLimitError::Request(error) => MempoolError::Http(error),
    }
}

fn error_for_status(response: Response) -> Result<Response, MempoolError> {
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        return Err(MempoolError::RateLimited);
    }
    Ok(response.error_for_status()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_segments_are_percent_encoded() {
        assert_eq!(encode_path_segment("bc1qexample"), "bc1qexample");
        assert_eq!(encode_path_segment("../address"), "..%2Faddress");
        assert_eq!(encode_path_segment("a?b#c"), "a%3Fb%23c");
        assert_eq!(encode_path_segment("a b"), "a%20b");
    }

    #[test]
    fn transaction_ids_must_be_hex_and_64_characters() {
        assert!(encode_txid("a".repeat(64).as_str()).is_ok());
        assert!(encode_txid("a".repeat(63).as_str()).is_err());
        assert!(encode_txid(&"z".repeat(64)).is_err());
        assert!(encode_txid("../../evil").is_err());
    }
}
