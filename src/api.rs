use color_eyre::{Result, eyre::eyre};
use serde::{Deserialize, Serialize};

use crate::config::api_base_url;

/// A callsign license record from the callrx-service API.
///
/// This mirrors the subset of the service's `CallsignResponse` that the CLI
/// displays and caches. The service returns additional fields (email, phone,
/// GMRS service, sibling licenses, …); `serde` ignores anything not declared
/// here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallsignRecord {
    pub call_sign: Option<String>,
    pub display_name: String,

    /// `Individual` or `Club`.
    pub license_type: Option<String>,
    /// FCC status code: `A`, `C`, `E`, `T`, `L`.
    pub license_status: String,
    /// Human-readable status, e.g. `Active`, `Expired`.
    pub license_status_label: String,

    /// Full operator-class name, e.g. `Amateur Extra` (null for club licenses).
    pub operator_class_label: Option<String>,

    pub previous_callsign: Option<String>,

    pub trustee_callsign: Option<String>,
    pub trustee_name: Option<String>,

    #[serde(default)]
    pub address: AddressInfo,

    pub frn: Option<String>,

    /// ISO 8601 dates (`YYYY-MM-DD`).
    pub grant_date: Option<String>,
    pub expired_date: Option<String>,
    pub last_action_date: Option<String>,

    pub uls_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AddressInfo {
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub zip_code: Option<String>,
    pub po_box: Option<String>,
}

/// Fetch a callsign record from the callrx-service backend.
pub fn lookup_callsign(callsign: &str) -> Result<CallsignRecord> {
    let url = format!("{}/callsign/{callsign}", api_base_url());

    let response = minreq::get(&url)
        .with_header("User-Agent", concat!("callrx/", env!("CARGO_PKG_VERSION")))
        .with_timeout(10)
        .send()
        .map_err(|e| eyre!("Network error: {e}"))?;

    match response.status_code {
        200 => {}
        404 => {
            return Err(eyre!(
                "Callsign '{callsign}' was not found in the FCC ULS database."
            ));
        }
        429 => {
            return Err(eyre!(
                "Rate limit exceeded — please wait a moment and try again."
            ));
        }
        code => {
            return Err(eyre!(
                "HTTP {code} from the callrx-service API — the service may be temporarily unavailable."
            ));
        }
    }

    let record: CallsignRecord = response
        .json()
        .map_err(|e| eyre!("Failed to parse response: {e}"))?;

    Ok(record)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

impl CallsignRecord {
    /// Returns the canonical callsign from the record.
    pub fn callsign(&self) -> &str {
        self.call_sign.as_deref().unwrap_or("—")
    }

    /// Returns the human-readable license class.
    ///
    /// The service already emits full operator-class names (`Amateur Extra`,
    /// `General`, …); this returns it verbatim, or `—` when absent (e.g. club
    /// licenses, which have a trustee instead of an operator class).
    pub fn license_class_label(&self) -> &str {
        self.operator_class_label.as_deref().unwrap_or("—")
    }

    /// Returns true if the license is expired, per the FCC status code.
    pub fn is_expired(&self) -> bool {
        self.license_status == "E"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_with(class_label: Option<&str>, status: &str) -> CallsignRecord {
        CallsignRecord {
            call_sign: Some("W1AW".to_string()),
            display_name: "ARRL HQ OPERATORS CLUB".to_string(),
            license_type: Some("Club".to_string()),
            license_status: status.to_string(),
            license_status_label: "Active".to_string(),
            operator_class_label: class_label.map(str::to_string),
            previous_callsign: None,
            trustee_callsign: None,
            trustee_name: None,
            address: AddressInfo::default(),
            frn: None,
            grant_date: None,
            expired_date: None,
            last_action_date: None,
            uls_url: None,
        }
    }

    #[test]
    fn class_label_returns_service_label_verbatim() {
        assert_eq!(
            record_with(Some("Amateur Extra"), "A").license_class_label(),
            "Amateur Extra"
        );
        assert_eq!(
            record_with(Some("General"), "A").license_class_label(),
            "General"
        );
    }

    #[test]
    fn class_label_handles_missing() {
        assert_eq!(record_with(None, "A").license_class_label(), "—");
    }

    #[test]
    fn is_expired_follows_status_code() {
        assert!(record_with(None, "E").is_expired());
        assert!(!record_with(None, "A").is_expired());
        assert!(!record_with(None, "C").is_expired());
    }

    #[test]
    fn callsign_falls_back_when_absent() {
        let mut rec = record_with(None, "A");
        assert_eq!(rec.callsign(), "W1AW");
        rec.call_sign = None;
        assert_eq!(rec.callsign(), "—");
    }
}
