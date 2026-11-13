//! Runtime forwarding macros used at the top of every server function.
//!
//! The macro only expands inside a `#[server]` body in a native all-in-one
//! build. Web clients never compile the server body, and pure server builds
//! always resolve `BackendMode::Local`.

#[allow(unused_macros)]
macro_rules! remote_forward_no_body {
    (
        $method:ident,
        $path:literal,
        [$($path_arg:ident),* $(,)?],
        [$($query_arg:ident),* $(,)?]
    ) => {
        #[cfg(feature = "remote-backend")]
        {
            if $crate::backend::is_remote() {
                return $crate::api::remote::call(
                    ::dioxus::fullstack::Method::$method,
                    &format!($path $(, $path_arg)*),
                    Some(::serde_json::json!({
                        $(stringify!($query_arg): $query_arg),*
                    })),
                    None,
                )
                .await;
            }
        }
    };
}

#[allow(unused_macros)]
macro_rules! remote_forward_body {
    (
        $method:ident,
        $path:literal,
        [$($path_arg:ident),* $(,)?],
        [$($query_arg:ident),* $(,)?],
        [$($body_arg:ident),* $(,)?]
    ) => {
        #[cfg(feature = "remote-backend")]
        {
            if $crate::backend::is_remote() {
                return $crate::api::remote::call(
                    ::dioxus::fullstack::Method::$method,
                    &format!($path $(, $path_arg)*),
                    Some(::serde_json::json!({
                        $(stringify!($query_arg): $query_arg),*
                    })),
                    Some(::serde_json::json!({
                        $(stringify!($body_arg): $body_arg),*
                    })),
                )
                .await;
            }
        }
    };
}

#[allow(unused_macros)]
macro_rules! remote_get {
    ($path:literal, [$($path_arg:ident),* $(,)?], [$($query_arg:ident),* $(,)?]) => {
        remote_forward_no_body!(GET, $path, [$($path_arg),*], [$($query_arg),*]);
    };
}

#[allow(unused_macros)]
macro_rules! remote_delete {
    ($path:literal, [$($path_arg:ident),* $(,)?], [$($query_arg:ident),* $(,)?]) => {
        remote_forward_no_body!(DELETE, $path, [$($path_arg),*], [$($query_arg),*]);
    };
}

#[allow(unused_macros)]
macro_rules! remote_post {
    (
        $path:literal,
        [$($path_arg:ident),* $(,)?],
        [$($query_arg:ident),* $(,)?],
        [$($body_arg:ident),* $(,)?]
    ) => {
        remote_forward_body!(POST, $path, [$($path_arg),*], [$($query_arg),*], [$($body_arg),*]);
    };
}

#[allow(unused_macros)]
macro_rules! remote_post_with_timeout {
    (
        $path:literal,
        [$($path_arg:ident),* $(,)?],
        [$($query_arg:ident),* $(,)?],
        [$($body_arg:ident),* $(,)?],
        $timeout:expr
    ) => {
        #[cfg(feature = "remote-backend")]
        {
            if $crate::backend::is_remote() {
                return $crate::api::remote::call_with_timeout(
                    ::dioxus::fullstack::Method::POST,
                    &format!($path $(, $path_arg)*),
                    Some(::serde_json::json!({
                        $(stringify!($query_arg): $query_arg),*
                    })),
                    Some(::serde_json::json!({
                        $(stringify!($body_arg): $body_arg),*
                    })),
                    $timeout,
                )
                .await;
            }
        }
    };
}

/// Forward an encryption call that returns an `UnlockResult` and remember the
/// session token issued by the remote server.
#[allow(unused_macros)]
macro_rules! remote_post_session {
    ($path:literal, $body:tt) => {
        #[cfg(feature = "remote-backend")]
        {
            if $crate::backend::is_remote() {
                let result: $crate::domain::encryption::UnlockResult = $crate::api::remote::call(
                    ::dioxus::fullstack::Method::POST,
                    $path,
                    Some(::serde_json::json!({})),
                    Some(::serde_json::json!($body)),
                )
                .await?;
                if let Some(token) = result.session_token.as_deref() {
                    $crate::backend::set_session_token(token);
                }
                return Ok(result);
            }
        }
    };
}

#[allow(unused_macros)]
macro_rules! remote_put {
    (
        $path:literal,
        [$($path_arg:ident),* $(,)?],
        [$($query_arg:ident),* $(,)?],
        [$($body_arg:ident),* $(,)?]
    ) => {
        remote_forward_body!(PUT, $path, [$($path_arg),*], [$($query_arg),*], [$($body_arg),*]);
    };
}
