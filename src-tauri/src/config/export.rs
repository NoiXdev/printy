use super::schema::{ConfigExport, ExportedFolder, ExportedSettings, SCHEMA_VERSION};
use crate::db::{folders, settings, Db};

/// Builds the transferable slice of this machine's configuration -- see
/// `schema::ExportedFolder` / `schema::ExportedSettings` for exactly what is
/// carried and, in their doc comments, why the rest is deliberately left out.
pub async fn export_config(db: &Db) -> Result<ConfigExport, sqlx::Error> {
    let rows = folders::list_folders(db).await?;
    let exported_folders = rows
        .into_iter()
        .map(|f| ExportedFolder {
            file_types: f.types(),
            name: f.name,
            path: f.path,
            enabled: f.enabled != 0,
            poll_interval_secs: f.poll_interval_secs,
            printer_name: f.printer_name,
            copies: f.copies,
            duplex: f.duplex,
            color_mode: f.color_mode,
            post_action: f.post_action,
            fit_to_page: f.fit_to_page != 0,
        })
        .collect();

    let notification_mode = settings::get_setting(db, "notification_mode")
        .await?
        .unwrap_or_else(|| "all".into());
    let autostart = settings::get_setting(db, "autostart").await?.as_deref() == Some("1");
    let start_minimized = settings::start_minimized(db).await;
    let default_poll_interval_secs = settings::default_poll_interval_secs(db).await;

    Ok(ConfigExport {
        schema_version: SCHEMA_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        folders: exported_folders,
        settings: ExportedSettings {
            notification_mode,
            autostart,
            start_minimized,
            default_poll_interval_secs,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::SCHEMA_VERSION;
    use crate::db::connect;
    use crate::db::folders::{create_folder, NewFolder};
    use crate::db::settings::set_setting;

    async fn sample_folder(db: &Db, name: &str, path: &str) {
        create_folder(db, &NewFolder {
            name: name.into(),
            path: path.into(),
            poll_interval_secs: 7,
            file_types: vec!["pdf".into(), "png".into()],
            printer_name: "Brother".into(),
            copies: 2,
            duplex: "long_edge".into(),
            color_mode: "color".into(),
            post_action: "delete".into(),
            fit_to_page: false,
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn exports_every_folder_field_the_spec_lists() {
        let db = connect("sqlite::memory:").await.unwrap();
        sample_folder(&db, "Scans", "/tmp/printy-export-scans").await;

        let export = export_config(&db).await.unwrap();
        assert_eq!(export.schema_version, SCHEMA_VERSION);
        assert_eq!(export.app_version, env!("CARGO_PKG_VERSION"));
        assert!(!export.exported_at.is_empty());

        assert_eq!(export.folders.len(), 1);
        let f = &export.folders[0];
        assert_eq!(f.name, "Scans");
        assert_eq!(f.path, "/tmp/printy-export-scans");
        assert!(f.enabled);
        assert_eq!(f.poll_interval_secs, 7);
        assert_eq!(f.file_types, vec!["pdf".to_string(), "png".to_string()]);
        assert_eq!(f.printer_name, "Brother");
        assert_eq!(f.copies, 2);
        assert_eq!(f.duplex, "long_edge");
        assert_eq!(f.color_mode, "color");
        assert_eq!(f.post_action, "delete");
        assert!(!f.fit_to_page);
    }

    #[tokio::test]
    async fn exports_the_four_settings_with_their_defaults() {
        let db = connect("sqlite::memory:").await.unwrap();
        let export = export_config(&db).await.unwrap();
        assert_eq!(export.settings.notification_mode, "all");
        assert!(!export.settings.autostart);
        assert!(!export.settings.start_minimized);
        assert_eq!(export.settings.default_poll_interval_secs, 3);
    }

    #[tokio::test]
    async fn exports_non_default_settings_values() {
        let db = connect("sqlite::memory:").await.unwrap();
        set_setting(&db, "notification_mode", "errors").await.unwrap();
        set_setting(&db, "autostart", "1").await.unwrap();
        set_setting(&db, "start_minimized", "1").await.unwrap();
        set_setting(&db, "default_poll_interval_secs", "9").await.unwrap();

        let export = export_config(&db).await.unwrap();
        assert_eq!(export.settings.notification_mode, "errors");
        assert!(export.settings.autostart);
        assert!(export.settings.start_minimized);
        assert_eq!(export.settings.default_poll_interval_secs, 9);
    }

    /// The excluded fields are structurally absent from `ExportedSettings` /
    /// `ExportedFolder`, but this proves it at the JSON level too: even with
    /// sensitive values set, the serialized output never contains them.
    #[tokio::test]
    async fn the_serialized_json_never_contains_the_excluded_fields() {
        let db = connect("sqlite::memory:").await.unwrap();
        sample_folder(&db, "Scans", "/tmp/printy-export-excluded").await;
        set_setting(&db, "pdfium_path", "/opt/pdfium/libpdfium.dylib").await.unwrap();
        set_setting(&db, "sumatra_path", "/opt/sumatra/SumatraPDF").await.unwrap();
        set_setting(&db, "user_paused", "1").await.unwrap();

        let export = export_config(&db).await.unwrap();
        let json = serde_json::to_string(&export).unwrap();
        assert!(!json.contains("pdfium"));
        assert!(!json.contains("sumatra"));
        assert!(!json.contains("user_paused"));
        assert!(!json.contains("\"status\""));
    }

    #[tokio::test]
    async fn exports_multiple_folders_in_id_order() {
        let db = connect("sqlite::memory:").await.unwrap();
        sample_folder(&db, "First", "/tmp/printy-export-first").await;
        sample_folder(&db, "Second", "/tmp/printy-export-second").await;

        let export = export_config(&db).await.unwrap();
        assert_eq!(export.folders.len(), 2);
        assert_eq!(export.folders[0].name, "First");
        assert_eq!(export.folders[1].name, "Second");
    }
}
