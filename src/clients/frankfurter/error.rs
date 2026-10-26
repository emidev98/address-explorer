use thiserror::Error;

#[derive(Debug, Error)]
pub enum FrankfurterError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}
