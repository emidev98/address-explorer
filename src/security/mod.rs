//! Server-side security primitives: TLS transport, session authorization,
//! security policy, request limits, file permissions, and process hardening.

#[cfg(all(feature = "server", not(any(feature = "desktop", feature = "mobile"))))]
pub mod body_limit;
#[cfg(feature = "server")]
pub mod files;
#[cfg(feature = "server")]
pub mod hardening;
#[cfg(all(feature = "server", not(any(feature = "desktop", feature = "mobile"))))]
pub mod headers;
#[cfg(feature = "server")]
pub mod policy;
#[cfg(feature = "server")]
pub mod rate_limit;
#[cfg(feature = "server")]
pub mod session;
#[cfg(all(feature = "server", not(any(feature = "desktop", feature = "mobile"))))]
pub mod tls;
