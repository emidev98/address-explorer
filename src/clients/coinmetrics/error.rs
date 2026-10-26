use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoinMetricsError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    /// Error returned by the API itself, for example a forbidden asset or an
    /// invalid parameter.
    #[error("coin metrics api error ({status} {error_type}): {message}")]
    Api {
        status: u16,
        error_type: String,
        message: String,
    },
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid url: {0}")]
    Url(String),
}
