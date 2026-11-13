//! Bounded response reads for third-party HTTP clients.

/// Default maximum body accepted from an external provider.
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// Read a response while enforcing a hard size cap. A missing or lying
/// `Content-Length` cannot bypass the cap because chunks are counted too.
pub async fn read_limited(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, ResponseLimitError> {
    if let Some(length) = response.content_length() {
        if length > max_bytes as u64 {
            return Err(ResponseLimitError::TooLarge);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(ResponseLimitError::Request)?
        {
            if bytes.len().saturating_add(chunk.len()) > max_bytes {
                return Err(ResponseLimitError::TooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let bytes = response
            .bytes()
            .await
            .map_err(ResponseLimitError::Request)?;
        if bytes.len() > max_bytes {
            return Err(ResponseLimitError::TooLarge);
        }
        Ok(bytes.to_vec())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResponseLimitError {
    #[error("the external response exceeded the configured size limit")]
    TooLarge,
    #[error("could not read the external response: {0}")]
    Request(#[from] reqwest::Error),
}
