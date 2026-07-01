use color_eyre::{Result, eyre::eyre};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::config::api_base_url;

/// A callsign license record from the callrx-service API.
///
/// This mirrors the subset of the service's `CallsignResponse` that the CLI
/// displays and caches. The service returns a few additional fields (raw
/// first/last/mi/suffix name parts); `serde` ignores anything not declared
/// here since `display_name` already composes them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CallsignRecord {
    pub unique_system_identifier: Option<i64>,
    pub call_sign: Option<String>,
    pub display_name: String,

    /// `Individual` or `Club`.
    pub license_type: Option<String>,
    /// FCC status code: `A`, `C`, `E`, `T`, `L`.
    pub license_status: String,
    /// Human-readable status, e.g. `Active`, `Expired`.
    pub license_status_label: String,

    /// FCC class code (`E`/`A`/`G`/`T`/`N`); null for club licenses.
    pub operator_class: Option<String>,
    /// Full operator-class name, e.g. `Amateur Extra` (null for club licenses).
    pub operator_class_label: Option<String>,
    /// FCC callsign group (A=Extra, B=Advanced, C=General, D=Technician/Novice).
    pub group_code: Option<String>,
    /// FCC geographic call district (1–10, or K/N/S for territories).
    pub region_code: Option<String>,

    pub previous_callsign: Option<String>,
    /// FCC class code before upgrade.
    pub previous_operator_class: Option<String>,
    pub previous_operator_class_label: Option<String>,

    pub trustee_callsign: Option<String>,
    pub trustee_name: Option<String>,
    /// `Y` if the callsign was obtained via the FCC vanity program.
    pub vanity_call_sign_change: Option<String>,
    /// `P`=previous holder, `R`=close relative, `S`=surviving spouse.
    pub vanity_relationship: Option<String>,

    #[serde(default)]
    pub address: AddressInfo,
    pub email: Option<String>,
    pub phone: Option<String>,

    pub frn: Option<String>,

    /// ISO 8601 dates (`YYYY-MM-DD`).
    pub grant_date: Option<String>,
    pub expired_date: Option<String>,
    pub cancellation_date: Option<String>,
    pub effective_date: Option<String>,
    pub last_action_date: Option<String>,

    pub uls_url: Option<String>,

    /// Radio service code: `A`=Amateur Radio, `G`=GMRS.
    pub service: Option<String>,
    pub service_label: Option<String>,

    /// Other licenses held by the same FCC registrant (same FRN).
    pub frn_licenses: Option<Vec<CallsignSummary>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AddressInfo {
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub zip_code: Option<String>,
    pub po_box: Option<String>,
}

/// Compact record used for FRN siblings and neighbors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallsignSummary {
    pub unique_system_identifier: Option<i64>,
    pub call_sign: Option<String>,
    pub display_name: String,
    pub license_status_label: String,
    pub license_type: Option<String>,
    pub operator_class_label: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub service: Option<String>,
    pub service_label: Option<String>,
}

/// A [`CallsignSummary`] plus the street address, used in neighbors results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborSummary {
    pub unique_system_identifier: Option<i64>,
    pub call_sign: Option<String>,
    pub display_name: String,
    pub license_status_label: String,
    pub license_type: Option<String>,
    pub operator_class_label: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub service: Option<String>,
    pub service_label: Option<String>,
    pub street_address: Option<String>,
}

/// Response from `GET /callsign/{callsign}/neighbors` — other active licensees
/// near the queried callsign's mailing address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborsResponse {
    pub call_sign: String,
    pub address_count: i64,
    pub address_results: Vec<NeighborSummary>,
    pub street_count: i64,
    pub street_results: Vec<NeighborSummary>,
}

/// Weather fields relevant to amateur radio operation, mirroring what the
/// web front-end renders (a subset of the service's fuller `WeatherInfo`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WeatherInfo {
    pub temperature_c: Option<f64>,
    pub temperature_f: Option<f64>,
    pub humidity_pct: Option<i64>,
    pub wind_speed_mph: Option<f64>,
    pub wind_direction_label: Option<String>,
    pub wind_gusts_mph: Option<f64>,
    pub pressure_hpa: Option<f64>,
    pub precipitation_mm: Option<f64>,
    pub cloud_cover_pct: Option<i64>,
    pub conditions: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocationTimeInfo {
    pub local_time: Option<String>,
    pub utc_time: Option<String>,
    pub timezone: Option<String>,
    pub utc_offset_seconds: Option<i64>,
}

/// Response from `GET /callsign/{callsign}/location-info` — local time and
/// weather at the licensee's mailing address.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocationInfoResponse {
    pub call_sign: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub time: Option<LocationTimeInfo>,
    pub weather: Option<WeatherInfo>,
}

fn get_json<T: DeserializeOwned>(path: &str, not_found: &str) -> Result<T> {
    let url = format!("{}{path}", api_base_url());

    let response = minreq::get(&url)
        .with_header("User-Agent", concat!("callrx/", env!("CARGO_PKG_VERSION")))
        .with_timeout(10)
        .send()
        .map_err(|e| eyre!("Network error: {e}"))?;

    match response.status_code {
        200 => {}
        404 => return Err(eyre!("{not_found}")),
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

    response
        .json()
        .map_err(|e| eyre!("Failed to parse response: {e}"))
}

/// Fetch a callsign record from the callrx-service backend.
pub fn lookup_callsign(callsign: &str) -> Result<CallsignRecord> {
    get_json(
        &format!("/callsign/{callsign}"),
        &format!("Callsign '{callsign}' was not found in the FCC ULS database."),
    )
}

/// Fetch other active licensees near the callsign's mailing address.
pub fn lookup_neighbors(callsign: &str) -> Result<NeighborsResponse> {
    get_json(
        &format!("/callsign/{callsign}/neighbors"),
        &format!("Callsign '{callsign}' was not found in the FCC ULS database."),
    )
}

/// Fetch local time and weather at the callsign's mailing address.
pub fn lookup_location_info(callsign: &str) -> Result<LocationInfoResponse> {
    get_json(
        &format!("/callsign/{callsign}/location-info"),
        &format!("Callsign '{callsign}' was not found in the FCC ULS database."),
    )
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
            ..Default::default()
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
