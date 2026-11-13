use thiserror::Error;
#[derive(Debug, Error)]
pub enum MempoolError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// The API answered HTTP 429: the request budget is exhausted and the
    /// caller should resume later instead of retrying immediately.
    #[error("mempool API rate limit exceeded")]
    RateLimited,
    #[error("invalid mempool request path value")]
    InvalidPath,
    #[error("the mempool response exceeded the size limit")]
    ResponseTooLarge,
}
