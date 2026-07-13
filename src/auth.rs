//! Local storage for a device-login credential (`callrx auth login`).
//!
//! Mirrors cache.rs's `CALLRX_CACHE_DIR`-override-then-`dirs`-crate pattern,
//! but resolves to a *config* directory, not a cache directory — this file
//! holds a live secret, not disposable API response data, so it's written
//! with 0600 permissions on Unix and never treated as safe to delete/rebuild.

use color_eyre::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredential {
    pub api_key: String,
    pub key_prefix: String,
    pub tier: String,
    pub created_at: String,
}

pub fn credentials_path() -> Option<PathBuf> {
    // CALLRX_CONFIG_DIR overrides the default system config directory.
    // Set via .env (loaded in debug builds) to redirect during dev, same as
    // CALLRX_CACHE_DIR in cache.rs.
    if let Ok(dir) = std::env::var("CALLRX_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join("credentials"));
    }
    dirs::config_dir().map(|d| d.join("callrx").join("credentials"))
}

/// Returns the stored credential, or `None` if not signed in, the file is
/// missing/unreadable, or its contents don't parse — any of these just means
/// "run `callrx auth login`", not a fatal error.
pub fn load() -> Option<StoredCredential> {
    let path = credentials_path()?;
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn save(cred: &StoredCredential) -> Result<()> {
    let path = credentials_path().ok_or_else(|| {
        color_eyre::eyre::eyre!("Could not determine a config directory to save credentials to")
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_vec_pretty(cred)?)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

pub fn clear() -> Result<()> {
    let Some(path) = credentials_path() else {
        return Ok(());
    };
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A single test function, not several `#[test]`s: cargo test runs tests
    // in parallel threads by default, and CALLRX_CONFIG_DIR is a process-wide
    // env var — two tests each setting/unsetting it concurrently would race.
    #[test]
    fn save_load_clear_round_trip() {
        let dir = std::env::temp_dir().join(format!("callrx-auth-test-{}", std::process::id()));
        // SAFETY: this is the only test in the binary that touches
        // CALLRX_CONFIG_DIR, so there's no concurrent reader/writer.
        unsafe { std::env::set_var("CALLRX_CONFIG_DIR", &dir) };

        assert!(clear().is_ok()); // clearing when nothing is stored is a no-op, not an error
        assert!(load().is_none());

        let cred = StoredCredential {
            api_key: "crx_abc123".to_string(),
            key_prefix: "crx_abc123de".to_string(),
            tier: "free".to_string(),
            created_at: "2026-01-01T00:00:00+00:00".to_string(),
        };
        save(&cred).unwrap();

        let loaded = load().unwrap();
        assert_eq!(loaded.api_key, cred.api_key);
        assert_eq!(loaded.key_prefix, cred.key_prefix);
        assert_eq!(loaded.tier, cred.tier);

        clear().unwrap();
        assert!(load().is_none());

        unsafe { std::env::remove_var("CALLRX_CONFIG_DIR") };
        let _ = std::fs::remove_dir_all(&dir);
    }
}
