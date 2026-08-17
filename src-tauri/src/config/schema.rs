use serde::{Deserialize, Serialize};

/// The only schema version this build understands. Bump this whenever the
/// exported shape changes in a way older code cannot read correctly, and
/// reject anything else on import rather than guessing.
pub const SCHEMA_VERSION: u32 = 1;

/// One `watch_folder` row, trimmed to what is worth carrying to another
/// machine. Deliberately excludes:
/// - `status` (`ok` / `path_missing`): runtime state of the *source* machine.
/// - print history and the dedup ledger (`print_job` rows): machine-bound,
///   large, and meaningless on another machine.
/// - `user_paused` lives on `ExportedSettings`, not here, but the same
///   reasoning applies to the folder-level equivalent: there is no per-folder
///   pause flag today, so nothing further to exclude on that front.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportedFolder {
    pub name: String,
    pub path: String,
    pub enabled: bool,
    pub poll_interval_secs: i64,
    pub file_types: Vec<String>,
    pub printer_name: String,
    pub copies: i64,
    pub duplex: String,
    pub color_mode: String,
    pub post_action: String,
    pub fit_to_page: bool,
}

/// The subset of `app_setting` worth transferring. Deliberately excludes:
/// - `sumatra_path` / `pdfium_path`: absolute paths of the *source* machine.
///   Worse than merely wrong: a configured-but-invalid pdfium path raises a
///   hard error by design instead of falling back, so importing one could
///   break printing on the target machine outright.
/// - `user_paused`: importing a configuration must not silently pause the
///   target machine's printing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportedSettings {
    pub notification_mode: String,
    pub autostart: bool,
    pub start_minimized: bool,
    pub default_poll_interval_secs: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigExport {
    pub schema_version: u32,
    pub app_version: String,
    pub exported_at: String,
    pub folders: Vec<ExportedFolder>,
    pub settings: ExportedSettings,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigParseError {
    #[error("Die Konfigurationsdatei ist beschädigt oder hat kein gültiges Format: {0}")]
    Malformed(String),
    #[error(
        "Diese Konfigurationsdatei hat Format-Version {found}, aber diese Version von Printy \
         versteht nur Version {expected}. Bitte mit einer passenden Printy-Version exportieren."
    )]
    UnsupportedVersion { found: u32, expected: u32 },
}

/// Parses and validates an exported configuration. `schema_version` is
/// checked *before* the rest of the shape is deserialized, so a version this
/// build cannot understand always produces the clear version error, never a
/// generic "field X missing" from a future format.
pub fn parse_export(json: &str) -> Result<ConfigExport, ConfigParseError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| ConfigParseError::Malformed(e.to_string()))?;

    let found = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            ConfigParseError::Malformed("Feld \"schema_version\" fehlt oder ist ungültig".into())
        })? as u32;

    if found != SCHEMA_VERSION {
        return Err(ConfigParseError::UnsupportedVersion { found, expected: SCHEMA_VERSION });
    }

    serde_json::from_value(value).map_err(|e| ConfigParseError::Malformed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ConfigExport {
        ConfigExport {
            schema_version: SCHEMA_VERSION,
            app_version: "0.1.0".into(),
            exported_at: "2026-08-17T10:00:00Z".into(),
            folders: vec![ExportedFolder {
                name: "Scans".into(),
                path: "/tmp/printy-scans".into(),
                enabled: true,
                poll_interval_secs: 5,
                file_types: vec!["pdf".into()],
                printer_name: "Brother".into(),
                copies: 1,
                duplex: "simplex".into(),
                color_mode: "mono".into(),
                post_action: "move".into(),
                fit_to_page: true,
            }],
            settings: ExportedSettings {
                notification_mode: "all".into(),
                autostart: false,
                start_minimized: false,
                default_poll_interval_secs: 3,
            },
        }
    }

    #[test]
    fn parses_a_well_formed_export_round_trip() {
        let json = serde_json::to_string(&sample()).unwrap();
        let parsed = parse_export(&json).unwrap();
        assert_eq!(parsed, sample());
    }

    #[test]
    fn rejects_malformed_json_without_panicking() {
        let err = parse_export("not json at all { ").unwrap_err();
        assert!(matches!(err, ConfigParseError::Malformed(_)));
    }

    #[test]
    fn rejects_a_missing_schema_version_field() {
        let err = parse_export(r#"{"folders": []}"#).unwrap_err();
        assert!(matches!(err, ConfigParseError::Malformed(_)));
    }

    #[test]
    fn rejects_a_newer_schema_version_with_both_numbers_in_the_message() {
        let mut value = serde_json::to_value(sample()).unwrap();
        value["schema_version"] = serde_json::json!(SCHEMA_VERSION + 1);
        let json = serde_json::to_string(&value).unwrap();

        let err = parse_export(&json).unwrap_err();
        let message = err.to_string();
        match err {
            ConfigParseError::UnsupportedVersion { found, expected } => {
                assert_eq!(found, SCHEMA_VERSION + 1);
                assert_eq!(expected, SCHEMA_VERSION);
            }
            other => panic!("expected UnsupportedVersion, got {other:?}"),
        }
        assert!(message.contains(&(SCHEMA_VERSION + 1).to_string()));
        assert!(message.contains(&SCHEMA_VERSION.to_string()));
    }

    #[test]
    fn rejects_an_unknown_schema_version_number() {
        let mut value = serde_json::to_value(sample()).unwrap();
        value["schema_version"] = serde_json::json!(999);
        let json = serde_json::to_string(&value).unwrap();

        assert!(matches!(
            parse_export(&json).unwrap_err(),
            ConfigParseError::UnsupportedVersion { found: 999, .. }
        ));
    }
}
