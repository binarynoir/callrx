use color_eyre::{Result, eyre::eyre};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::{
    AddressInfo, CallsignRecord, CurrentInfo, LocationInfo, OtherInfo, PreviousInfo, TrusteeInfo,
};

const SCHEMA_VERSION: i32 = 1;

/// Cache TTL: 7 days, matching callook.info's weekly FCC bulk-data sync.
pub const TTL_SECS: u64 = 7 * 24 * 60 * 60;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn db_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("callrx").join("callrx.db"))
}

/// Opens (or creates) the cache database and applies the schema if needed.
///
/// Returns an error only when the cache directory cannot be determined or the
/// file cannot be opened. Callers should use `.ok()` and treat `None` as
/// "cache unavailable" rather than a fatal error.
pub fn open() -> Result<Connection> {
    let path = db_path().ok_or_else(|| eyre!("Could not determine cache directory"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path)?;
    apply_schema(&conn)?;
    Ok(conn)
}

fn apply_schema(conn: &Connection) -> Result<()> {
    // These pragmas are connection-level and must be set on every open.
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;

    let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS callsigns (
             id               INTEGER PRIMARY KEY,
             callsign         TEXT    NOT NULL UNIQUE,
             status           TEXT    NOT NULL,
             license_type     TEXT,
             oper_class       TEXT,
             prev_callsign    TEXT,
             prev_oper_class  TEXT,
             trustee_callsign TEXT,
             trustee_name     TEXT,
             name             TEXT,
             addr_line1       TEXT,
             addr_line2       TEXT,
             addr_attn        TEXT,
             latitude         TEXT,
             longitude        TEXT,
             gridsquare       TEXT,
             grant_date       TEXT,
             expiry_date      TEXT,
             last_action_date TEXT,
             frn              TEXT,
             uls_url          TEXT,
             cached_at        INTEGER NOT NULL
         );

         CREATE TABLE IF NOT EXISTS lookup_history (
             id           INTEGER PRIMARY KEY,
             callsign     TEXT    NOT NULL,
             looked_up_at INTEGER NOT NULL,
             source       TEXT    NOT NULL CHECK(source IN ('api', 'cache'))
         );

         CREATE INDEX IF NOT EXISTS idx_lookup_history_callsign
             ON lookup_history(callsign);
         CREATE INDEX IF NOT EXISTS idx_lookup_history_looked_up_at
             ON lookup_history(looked_up_at);",
    )?;

    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

/// Returns the cached record and its `cached_at` Unix timestamp when a fresh
/// entry (within `TTL_SECS`) exists for `callsign`.
pub fn get(conn: &Connection, callsign: &str) -> Option<(CallsignRecord, u64)> {
    let min_age = now_secs().saturating_sub(TTL_SECS);

    conn.query_row(
        "SELECT status, license_type, oper_class,
                prev_callsign, prev_oper_class,
                trustee_callsign, trustee_name,
                name,
                addr_line1, addr_line2, addr_attn,
                latitude, longitude, gridsquare,
                grant_date, expiry_date, last_action_date, frn, uls_url,
                cached_at
         FROM   callsigns
         WHERE  callsign = ?1 AND cached_at >= ?2",
        params![callsign, min_age as i64],
        |row| {
            let status: String = row.get(0)?;
            let license_type: Option<String> = row.get(1)?;
            let oper_class: Option<String> = row.get(2)?;
            let prev_callsign: Option<String> = row.get(3)?;
            let prev_oper_class: Option<String> = row.get(4)?;
            let trustee_callsign: Option<String> = row.get(5)?;
            let trustee_name: Option<String> = row.get(6)?;
            let name: Option<String> = row.get(7)?;
            let addr_line1: Option<String> = row.get(8)?;
            let addr_line2: Option<String> = row.get(9)?;
            let addr_attn: Option<String> = row.get(10)?;
            let latitude: Option<String> = row.get(11)?;
            let longitude: Option<String> = row.get(12)?;
            let gridsquare: Option<String> = row.get(13)?;
            let grant_date: Option<String> = row.get(14)?;
            let expiry_date: Option<String> = row.get(15)?;
            let last_action_date: Option<String> = row.get(16)?;
            let frn: Option<String> = row.get(17)?;
            let uls_url: Option<String> = row.get(18)?;
            let cached_at: i64 = row.get(19)?;

            Ok((
                CallsignRecord {
                    status,
                    license_type,
                    current: Some(CurrentInfo {
                        callsign: Some(callsign.to_string()),
                        oper_class,
                    }),
                    previous: prev_callsign.map(|c| PreviousInfo {
                        callsign: Some(c),
                        oper_class: prev_oper_class,
                    }),
                    trustee: trustee_callsign.map(|c| TrusteeInfo {
                        callsign: Some(c),
                        name: trustee_name,
                    }),
                    name,
                    address: Some(AddressInfo {
                        line1: addr_line1,
                        line2: addr_line2,
                        attn: addr_attn,
                    }),
                    location: Some(LocationInfo {
                        latitude,
                        longitude,
                        gridsquare,
                    }),
                    other_info: Some(OtherInfo {
                        grant_date,
                        expiry_date,
                        last_action_date,
                        frn,
                        uls_url,
                    }),
                },
                cached_at as u64,
            ))
        },
    )
    .optional()
    .ok()
    .flatten()
}

/// Upserts a callsign record into the cache, updating `cached_at` to now.
pub fn store(conn: &Connection, record: &CallsignRecord) -> Result<()> {
    let callsign = record.callsign();
    let now = now_secs() as i64;
    let current = record.current.as_ref();
    let prev = record.previous.as_ref();
    let trustee = record.trustee.as_ref();
    let addr = record.address.as_ref();
    let loc = record.location.as_ref();
    let info = record.other_info.as_ref();

    conn.execute(
        "INSERT INTO callsigns (
             callsign, status, license_type, oper_class,
             prev_callsign, prev_oper_class, trustee_callsign, trustee_name, name,
             addr_line1, addr_line2, addr_attn, latitude, longitude, gridsquare,
             grant_date, expiry_date, last_action_date, frn, uls_url, cached_at
         ) VALUES (
             ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21
         )
         ON CONFLICT(callsign) DO UPDATE SET
             status           = excluded.status,
             license_type     = excluded.license_type,
             oper_class       = excluded.oper_class,
             prev_callsign    = excluded.prev_callsign,
             prev_oper_class  = excluded.prev_oper_class,
             trustee_callsign = excluded.trustee_callsign,
             trustee_name     = excluded.trustee_name,
             name             = excluded.name,
             addr_line1       = excluded.addr_line1,
             addr_line2       = excluded.addr_line2,
             addr_attn        = excluded.addr_attn,
             latitude         = excluded.latitude,
             longitude        = excluded.longitude,
             gridsquare       = excluded.gridsquare,
             grant_date       = excluded.grant_date,
             expiry_date      = excluded.expiry_date,
             last_action_date = excluded.last_action_date,
             frn              = excluded.frn,
             uls_url          = excluded.uls_url,
             cached_at        = excluded.cached_at",
        params![
            callsign,
            &record.status,
            record.license_type.as_deref(),
            current.and_then(|c| c.oper_class.as_deref()),
            prev.and_then(|p| p.callsign.as_deref())
                .filter(|s| !s.is_empty()),
            prev.and_then(|p| p.oper_class.as_deref())
                .filter(|s| !s.is_empty()),
            trustee
                .and_then(|t| t.callsign.as_deref())
                .filter(|s| !s.is_empty()),
            trustee
                .and_then(|t| t.name.as_deref())
                .filter(|s| !s.is_empty()),
            record.name.as_deref(),
            addr.and_then(|a| a.line1.as_deref()),
            addr.and_then(|a| a.line2.as_deref()),
            addr.and_then(|a| a.attn.as_deref()),
            loc.and_then(|l| l.latitude.as_deref()),
            loc.and_then(|l| l.longitude.as_deref()),
            loc.and_then(|l| l.gridsquare.as_deref()),
            info.and_then(|i| i.grant_date.as_deref()),
            info.and_then(|i| i.expiry_date.as_deref()),
            info.and_then(|i| i.last_action_date.as_deref()),
            info.and_then(|i| i.frn.as_deref()),
            info.and_then(|i| i.uls_url.as_deref()),
            now,
        ],
    )?;
    Ok(())
}

/// Appends a lookup event to the history table.
pub fn record_lookup(conn: &Connection, callsign: &str, source: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO lookup_history (callsign, looked_up_at, source)
         VALUES (?1, ?2, ?3)",
        params![callsign, now_secs() as i64, source],
    )?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{SCHEMA_VERSION, TTL_SECS, apply_schema, get, now_secs, record_lookup, store};
    use crate::api::{
        AddressInfo, CallsignRecord, CurrentInfo, LocationInfo, OtherInfo, PreviousInfo,
        TrusteeInfo,
    };
    use rusqlite::Connection;

    fn make_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        apply_schema(&conn).unwrap();
        conn
    }

    fn sample_record(callsign: &str) -> CallsignRecord {
        CallsignRecord {
            status: "VALID".to_string(),
            license_type: Some("INDIVIDUAL".to_string()),
            current: Some(CurrentInfo {
                callsign: Some(callsign.to_string()),
                oper_class: Some("EXTRA".to_string()),
            }),
            previous: None,
            trustee: None,
            name: Some("TEST OPERATOR".to_string()),
            address: Some(AddressInfo {
                line1: Some("123 MAIN ST".to_string()),
                line2: Some("ANYTOWN, ST 00000".to_string()),
                attn: None,
            }),
            location: Some(LocationInfo {
                latitude: Some("40.0".to_string()),
                longitude: Some("-74.0".to_string()),
                gridsquare: Some("FN20".to_string()),
            }),
            other_info: Some(OtherInfo {
                grant_date: Some("01/01/2020".to_string()),
                expiry_date: Some("01/01/2030".to_string()),
                last_action_date: Some("01/01/2020".to_string()),
                frn: Some("1234567890".to_string()),
                uls_url: Some("http://example.com".to_string()),
            }),
        }
    }

    #[test]
    fn schema_sets_user_version() {
        let conn = make_conn();
        let v: i32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
    }

    #[test]
    fn schema_apply_is_idempotent() {
        let conn = make_conn();
        apply_schema(&conn).unwrap(); // second call must not error
        let v: i32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
    }

    #[test]
    fn get_returns_none_when_empty() {
        let conn = make_conn();
        assert!(get(&conn, "W1AW").is_none());
    }

    #[test]
    fn store_and_get_round_trip() {
        let conn = make_conn();
        store(&conn, &sample_record("W1AW")).unwrap();

        let (rec, _) = get(&conn, "W1AW").unwrap();
        assert_eq!(rec.status, "VALID");
        assert_eq!(rec.callsign(), "W1AW");
        assert_eq!(rec.license_class_label(), "Amateur Extra");
        assert_eq!(rec.name.as_deref(), Some("TEST OPERATOR"));
        assert_eq!(
            rec.address.as_ref().and_then(|a| a.line1.as_deref()),
            Some("123 MAIN ST")
        );
        assert_eq!(
            rec.location.as_ref().and_then(|l| l.gridsquare.as_deref()),
            Some("FN20")
        );
        assert_eq!(
            rec.other_info.as_ref().and_then(|i| i.frn.as_deref()),
            Some("1234567890")
        );
    }

    #[test]
    fn get_respects_ttl_and_rejects_stale_entry() {
        let conn = make_conn();
        store(&conn, &sample_record("W1AW")).unwrap();

        // Backdate cached_at to just beyond the TTL.
        let stale = (now_secs() - TTL_SECS - 1) as i64;
        conn.execute(
            "UPDATE callsigns SET cached_at = ?1 WHERE callsign = 'W1AW'",
            rusqlite::params![stale],
        )
        .unwrap();

        assert!(get(&conn, "W1AW").is_none());
    }

    #[test]
    fn get_accepts_entry_within_ttl() {
        let conn = make_conn();
        store(&conn, &sample_record("W1AW")).unwrap();

        // Backdate to just inside the TTL.
        let fresh = (now_secs() - TTL_SECS + 60) as i64;
        conn.execute(
            "UPDATE callsigns SET cached_at = ?1 WHERE callsign = 'W1AW'",
            rusqlite::params![fresh],
        )
        .unwrap();

        assert!(get(&conn, "W1AW").is_some());
    }

    #[test]
    fn get_returns_correct_cached_at_timestamp() {
        let conn = make_conn();
        let before = now_secs();
        store(&conn, &sample_record("W1AW")).unwrap();
        let after = now_secs();

        let (_, cached_at) = get(&conn, "W1AW").unwrap();
        assert!(cached_at >= before && cached_at <= after);
    }

    #[test]
    fn store_upserts_on_conflict() {
        let conn = make_conn();
        store(&conn, &sample_record("W1AW")).unwrap();

        let mut updated = sample_record("W1AW");
        updated.name = Some("UPDATED NAME".to_string());
        store(&conn, &updated).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM callsigns WHERE callsign = 'W1AW'",
                rusqlite::params![],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        let (rec, _) = get(&conn, "W1AW").unwrap();
        assert_eq!(rec.name.as_deref(), Some("UPDATED NAME"));
    }

    #[test]
    fn store_handles_all_none_optional_fields() {
        let conn = make_conn();
        let minimal = CallsignRecord {
            status: "VALID".to_string(),
            license_type: None,
            current: Some(CurrentInfo {
                callsign: Some("W1AW".to_string()),
                oper_class: None,
            }),
            previous: None,
            trustee: None,
            name: None,
            address: None,
            location: None,
            other_info: None,
        };
        store(&conn, &minimal).unwrap();

        let (rec, _) = get(&conn, "W1AW").unwrap();
        assert_eq!(rec.status, "VALID");
        assert!(rec.name.is_none());
        assert!(rec.license_type.is_none());
        assert!(rec.previous.is_none());
        assert!(rec.trustee.is_none());
        assert!(
            rec.address
                .as_ref()
                .and_then(|a| a.line1.as_deref())
                .is_none()
        );
    }

    #[test]
    fn previous_and_trustee_round_trip() {
        let conn = make_conn();
        let mut rec = sample_record("W1AW");
        rec.previous = Some(PreviousInfo {
            callsign: Some("KA1ABC".to_string()),
            oper_class: Some("GENERAL".to_string()),
        });
        rec.trustee = Some(TrusteeInfo {
            callsign: Some("K1ZZ".to_string()),
            name: Some("SUMNER, DAVID G".to_string()),
        });
        store(&conn, &rec).unwrap();

        let (got, _) = get(&conn, "W1AW").unwrap();
        let prev = got.previous.as_ref().unwrap();
        assert_eq!(prev.callsign.as_deref(), Some("KA1ABC"));
        assert_eq!(prev.oper_class.as_deref(), Some("GENERAL"));

        let trustee = got.trustee.as_ref().unwrap();
        assert_eq!(trustee.callsign.as_deref(), Some("K1ZZ"));
        assert_eq!(trustee.name.as_deref(), Some("SUMNER, DAVID G"));
    }

    #[test]
    fn empty_string_previous_callsign_is_stored_as_none() {
        let conn = make_conn();
        let mut rec = sample_record("W1AW");
        rec.previous = Some(PreviousInfo {
            callsign: Some(String::new()),
            oper_class: Some(String::new()),
        });
        store(&conn, &rec).unwrap();

        // Empty strings are filtered before storage; previous comes back as None.
        let (got, _) = get(&conn, "W1AW").unwrap();
        assert!(got.previous.is_none());
    }

    #[test]
    fn record_lookup_appends_history() {
        let conn = make_conn();
        store(&conn, &sample_record("W1AW")).unwrap();

        record_lookup(&conn, "W1AW", "api").unwrap();
        record_lookup(&conn, "W1AW", "cache").unwrap();
        record_lookup(&conn, "W1AW", "api").unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lookup_history WHERE callsign = 'W1AW'",
                rusqlite::params![],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn record_lookup_is_independent_of_callsigns_table() {
        // lookup_history has no FK to callsigns, so history can be written
        // even when there is no cached record (e.g. before the cache is populated).
        let conn = make_conn();
        record_lookup(&conn, "W1AW", "api").unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lookup_history WHERE callsign = 'W1AW'",
                rusqlite::params![],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn lookup_history_records_correct_source() {
        let conn = make_conn();
        record_lookup(&conn, "W1AW", "api").unwrap();
        record_lookup(&conn, "W1AW", "cache").unwrap();

        let sources: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT source FROM lookup_history WHERE callsign = 'W1AW' ORDER BY id")
                .unwrap();
            stmt.query_map(rusqlite::params![], |r| r.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };
        assert_eq!(sources, vec!["api", "cache"]);
    }
}
