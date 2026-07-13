//! Resolves the base URL of the callrx-service backend.
//!
//! Published binaries should "just work" without any configuration, while local
//! development and self-hosters can point the CLI at their own service. The URL
//! is resolved in this order:
//!
//! 1. `CALLRX_API_URL` environment variable (runtime). In debug builds this is
//!    loaded from `.env` by `dotenvy` in `main`, so `cargo run` targets the dev
//!    service automatically.
//! 2. `CALLRX_API_URL` baked in at compile time via `option_env!`. The release
//!    workflow supplies this from a GitHub secret so the production endpoint is
//!    not committed to the (public) source tree.
//! 3. A `http://localhost:8073` fallback — the service's documented dev port.

const FALLBACK_API_URL: &str = "http://localhost:8073";

/// Returns the callrx-service base URL with any trailing slash removed.
pub fn api_base_url() -> String {
    if let Ok(url) = std::env::var("CALLRX_API_URL") {
        let url = url.trim();
        if !url.is_empty() {
            return trim_trailing_slash(url);
        }
    }

    if let Some(url) = option_env!("CALLRX_API_URL")
        .map(str::trim)
        .filter(|u| !u.is_empty())
    {
        return trim_trailing_slash(url);
    }

    FALLBACK_API_URL.to_string()
}

fn trim_trailing_slash(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

/// Returns the callrx-service API key, if one is available.
///
/// Resolution order:
/// 1. `CALLRX_API_KEY` environment variable — highest precedence, so CI/
///    automation can always override whatever's stored locally.
/// 2. The credential saved by `callrx auth login` (see `auth.rs`).
///
/// Never baked into the binary via `option_env!` like `CALLRX_API_URL`:
/// `callrx` is a publicly distributed binary, and an embedded personal key
/// would be trivially extractable from every release build.
pub fn api_key() -> Option<String> {
    if let Ok(key) = std::env::var("CALLRX_API_KEY") {
        let key = key.trim();
        if !key.is_empty() {
            return Some(key.to_string());
        }
    }

    crate::auth::load().map(|cred| cred.api_key)
}

#[cfg(test)]
mod tests {
    use super::trim_trailing_slash;

    #[test]
    fn trailing_slash_is_removed() {
        assert_eq!(trim_trailing_slash("http://x/"), "http://x");
        assert_eq!(trim_trailing_slash("http://x///"), "http://x");
    }

    #[test]
    fn url_without_trailing_slash_is_unchanged() {
        assert_eq!(trim_trailing_slash("http://x:8073"), "http://x:8073");
    }
}
