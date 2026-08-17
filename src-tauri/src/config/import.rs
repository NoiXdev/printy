use super::schema::{ConfigExport, ExportedFolder};
use crate::db::folders::{self, NewFolder};
use crate::db::settings;
use crate::db::Db;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportMode {
    /// Matches existing folders by `path`, updates the matches, creates the
    /// rest. Local folders whose path is not in the file are left alone.
    Merge,
    /// Deletes every existing folder first, then inserts everything from the
    /// file. Destructive -- the caller must confirm concretely before this
    /// runs (see the folder delete confirmation this mirrors).
    Replace,
}

/// Why a folder was force-disabled on import. Both can be true at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DisabledReason {
    pub path_missing: bool,
    pub printer_missing: bool,
}

impl DisabledReason {
    fn is_any(self) -> bool {
        self.path_missing || self.printer_missing
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FolderImportAction {
    Created,
    Updated,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderImportOutcome {
    pub name: String,
    pub path: String,
    pub action: FolderImportAction,
    pub enabled: bool,
    /// `Some` only when the safety net forced `enabled = false` -- never set
    /// just because the folder arrived disabled from the source machine by
    /// the user's own choice. This is the list the UI must show prominently.
    pub disabled_reason: Option<DisabledReason>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportReport {
    pub mode: ImportMode,
    pub total_folders: i64,
    pub created: i64,
    pub updated: i64,
    pub disabled: i64,
    pub settings_applied: i64,
    pub folders: Vec<FolderImportOutcome>,
}

fn to_new_folder(ef: &ExportedFolder) -> NewFolder {
    NewFolder {
        name: ef.name.clone(),
        path: ef.path.clone(),
        poll_interval_secs: ef.poll_interval_secs,
        file_types: ef.file_types.clone(),
        printer_name: ef.printer_name.clone(),
        copies: ef.copies,
        duplex: ef.duplex.clone(),
        color_mode: ef.color_mode.clone(),
        post_action: ef.post_action.clone(),
        fit_to_page: ef.fit_to_page,
    }
}

/// Never active by accident: a folder whose watched path no longer exists on
/// this machine, or whose printer is no longer installed, must never end up
/// enabled -- printing to the wrong device, or silently to nothing, is worse
/// than a folder the user has to notice and fix.
fn safety_check(path: &str, printer_name: &str, available_printers: &HashSet<String>) -> DisabledReason {
    DisabledReason {
        path_missing: !Path::new(path).is_dir(),
        printer_missing: !available_printers.contains(printer_name),
    }
}

async fn apply_settings(db: &Db, export: &ConfigExport) -> Result<i64, sqlx::Error> {
    settings::set_setting(db, "notification_mode", &export.settings.notification_mode).await?;
    settings::set_setting(
        db,
        "autostart",
        if export.settings.autostart { "1" } else { "0" },
    )
    .await?;
    settings::set_setting(
        db,
        "start_minimized",
        if export.settings.start_minimized { "1" } else { "0" },
    )
    .await?;
    settings::set_setting(
        db,
        "default_poll_interval_secs",
        &export.settings.default_poll_interval_secs.to_string(),
    )
    .await?;
    Ok(4)
}

/// Applies an imported configuration. `available_printers` is supplied by the
/// caller (the command layer, which actually talks to the print subsystem)
/// so this stays testable with a plain in-memory set.
pub async fn import_config(
    db: &Db,
    export: ConfigExport,
    mode: ImportMode,
    available_printers: &HashSet<String>,
) -> Result<ImportReport, sqlx::Error> {
    if mode == ImportMode::Replace {
        for f in folders::list_folders(db).await? {
            folders::delete_folder(db, f.id).await?;
        }
    }

    let existing_by_path: HashMap<String, i64> = if mode == ImportMode::Merge {
        folders::list_folders(db)
            .await?
            .into_iter()
            .map(|f| (f.path, f.id))
            .collect()
    } else {
        HashMap::new()
    };

    let mut outcomes = Vec::with_capacity(export.folders.len());
    let mut created = 0i64;
    let mut updated = 0i64;
    let mut disabled = 0i64;

    for ef in &export.folders {
        let new_folder = to_new_folder(ef);
        let (id, action) = match existing_by_path.get(&ef.path) {
            Some(&id) => {
                folders::update_folder(db, id, &new_folder).await?;
                (id, FolderImportAction::Updated)
            }
            None => {
                let row = folders::create_folder(db, &new_folder).await?;
                (row.id, FolderImportAction::Created)
            }
        };

        let reason = safety_check(&ef.path, &ef.printer_name, available_printers);
        let enabled = if reason.is_any() { false } else { ef.enabled };
        folders::set_folder_enabled(db, id, enabled).await?;

        match action {
            FolderImportAction::Created => created += 1,
            FolderImportAction::Updated => updated += 1,
        }
        if reason.is_any() {
            disabled += 1;
        }

        outcomes.push(FolderImportOutcome {
            name: ef.name.clone(),
            path: ef.path.clone(),
            action,
            enabled,
            disabled_reason: if reason.is_any() { Some(reason) } else { None },
        });
    }

    let settings_applied = apply_settings(db, &export).await?;

    Ok(ImportReport {
        mode,
        total_folders: export.folders.len() as i64,
        created,
        updated,
        disabled,
        settings_applied,
        folders: outcomes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::export::export_config;
    use crate::config::schema::{ExportedFolder, ExportedSettings, SCHEMA_VERSION};
    use crate::db::connect;
    use crate::db::folders::{create_folder, get_folder, list_folders, NewFolder};

    fn new_folder(name: &str, path: &str, printer: &str) -> NewFolder {
        NewFolder {
            name: name.into(),
            path: path.into(),
            poll_interval_secs: 5,
            file_types: vec!["pdf".into()],
            printer_name: printer.into(),
            copies: 1,
            duplex: "simplex".into(),
            color_mode: "mono".into(),
            post_action: "move".into(),
            fit_to_page: true,
        }
    }

    fn exported_folder(name: &str, path: &str, printer: &str) -> ExportedFolder {
        ExportedFolder {
            name: name.into(),
            path: path.into(),
            enabled: true,
            poll_interval_secs: 5,
            file_types: vec!["pdf".into()],
            printer_name: printer.into(),
            copies: 1,
            duplex: "simplex".into(),
            color_mode: "mono".into(),
            post_action: "move".into(),
            fit_to_page: true,
        }
    }

    fn export_with(folders: Vec<ExportedFolder>) -> ConfigExport {
        ConfigExport {
            schema_version: SCHEMA_VERSION,
            app_version: "0.1.0".into(),
            exported_at: "2026-08-17T10:00:00Z".into(),
            folders,
            settings: ExportedSettings {
                notification_mode: "errors".into(),
                autostart: true,
                start_minimized: true,
                default_poll_interval_secs: 9,
            },
        }
    }

    fn printers(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[tokio::test]
    async fn full_round_trip_export_wipe_import_matches_the_original() {
        let db = connect("sqlite::memory:").await.unwrap();
        // Existing dirs so the round-tripped folders come back enabled.
        let d1 = std::env::temp_dir().join("printy-import-roundtrip-a");
        let d2 = std::env::temp_dir().join("printy-import-roundtrip-b");
        std::fs::create_dir_all(&d1).unwrap();
        std::fs::create_dir_all(&d2).unwrap();

        create_folder(&db, &new_folder("A", d1.to_str().unwrap(), "Brother")).await.unwrap();
        create_folder(&db, &new_folder("B", d2.to_str().unwrap(), "Xerox")).await.unwrap();

        let exported = export_config(&db).await.unwrap();

        // Wipe the database clean (simulating a fresh machine).
        for f in list_folders(&db).await.unwrap() {
            folders::delete_folder(&db, f.id).await.unwrap();
        }
        assert!(list_folders(&db).await.unwrap().is_empty());

        let available = printers(&["Brother", "Xerox"]);
        let report =
            import_config(&db, exported.clone(), ImportMode::Merge, &available).await.unwrap();

        assert_eq!(report.total_folders, 2);
        assert_eq!(report.created, 2);
        assert_eq!(report.updated, 0);
        assert_eq!(report.disabled, 0);
        assert_eq!(report.settings_applied, 4);

        let after = export_config(&db).await.unwrap();
        assert_eq!(after.folders, exported.folders);
        assert_eq!(after.settings, exported.settings);

        std::fs::remove_dir_all(&d1).ok();
        std::fs::remove_dir_all(&d2).ok();
    }

    #[tokio::test]
    async fn merge_updates_a_matching_path_and_leaves_an_unrelated_local_folder_untouched() {
        let db = connect("sqlite::memory:").await.unwrap();
        let watched = std::env::temp_dir().join("printy-import-merge-watched");
        std::fs::create_dir_all(&watched).unwrap();

        let matched =
            create_folder(&db, &new_folder("Old name", watched.to_str().unwrap(), "OldPrinter"))
                .await
                .unwrap();
        let local_only =
            create_folder(&db, &new_folder("Local only", "/tmp/printy-local-only", "LocalPrinter"))
                .await
                .unwrap();

        let export = export_with(vec![exported_folder(
            "New name",
            watched.to_str().unwrap(),
            "NewPrinter",
        )]);

        let available = printers(&["NewPrinter"]);
        let report = import_config(&db, export, ImportMode::Merge, &available).await.unwrap();

        assert_eq!(report.created, 0);
        assert_eq!(report.updated, 1);
        assert_eq!(report.folders[0].action, FolderImportAction::Updated);

        let updated = get_folder(&db, matched.id).await.unwrap().unwrap();
        assert_eq!(updated.name, "New name");
        assert_eq!(updated.printer_name, "NewPrinter");

        // The unrelated local folder is completely untouched.
        let untouched = get_folder(&db, local_only.id).await.unwrap().unwrap();
        assert_eq!(untouched.name, "Local only");
        assert_eq!(untouched.printer_name, "LocalPrinter");
        assert_eq!(untouched.enabled, 1);

        std::fs::remove_dir_all(&watched).ok();
    }

    #[tokio::test]
    async fn replace_removes_a_folder_not_present_in_the_file() {
        let db = connect("sqlite::memory:").await.unwrap();
        create_folder(&db, &new_folder("Local only", "/tmp/printy-replace-local", "P"))
            .await
            .unwrap();

        let watched = std::env::temp_dir().join("printy-import-replace-watched");
        std::fs::create_dir_all(&watched).unwrap();
        let export =
            export_with(vec![exported_folder("From file", watched.to_str().unwrap(), "P")]);

        let available = printers(&["P"]);
        let report = import_config(&db, export, ImportMode::Replace, &available).await.unwrap();

        assert_eq!(report.created, 1);
        assert_eq!(report.updated, 0);

        let remaining = list_folders(&db).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].name, "From file");

        std::fs::remove_dir_all(&watched).ok();
    }

    #[tokio::test]
    async fn a_folder_with_a_nonexistent_path_arrives_disabled_with_the_right_reason() {
        let db = connect("sqlite::memory:").await.unwrap();
        let export = export_with(vec![exported_folder(
            "Gone",
            "/tmp/printy-does-not-exist-config-import",
            "Brother",
        )]);

        let available = printers(&["Brother"]);
        let report = import_config(&db, export, ImportMode::Merge, &available).await.unwrap();

        assert_eq!(report.disabled, 1);
        let outcome = &report.folders[0];
        assert!(!outcome.enabled);
        let reason = outcome.disabled_reason.expect("expected a disabled reason");
        assert!(reason.path_missing);
        assert!(!reason.printer_missing);

        let row = list_folders(&db).await.unwrap().into_iter().next().unwrap();
        assert_eq!(row.enabled, 0);
    }

    #[tokio::test]
    async fn a_folder_whose_printer_is_gone_arrives_disabled_with_the_right_reason() {
        let db = connect("sqlite::memory:").await.unwrap();
        let watched = std::env::temp_dir().join("printy-import-missing-printer");
        std::fs::create_dir_all(&watched).unwrap();
        let export = export_with(vec![exported_folder(
            "NoPrinter",
            watched.to_str().unwrap(),
            "Vanished Printer",
        )]);

        let available = printers(&["Brother"]);
        let report = import_config(&db, export, ImportMode::Merge, &available).await.unwrap();

        assert_eq!(report.disabled, 1);
        let reason = report.folders[0].disabled_reason.expect("expected a disabled reason");
        assert!(!reason.path_missing);
        assert!(reason.printer_missing);

        std::fs::remove_dir_all(&watched).ok();
    }

    #[tokio::test]
    async fn both_path_and_printer_missing_reports_both_reasons() {
        let db = connect("sqlite::memory:").await.unwrap();
        let export = export_with(vec![exported_folder(
            "Both gone",
            "/tmp/printy-does-not-exist-both-gone",
            "Vanished",
        )]);

        let available = printers(&["Brother"]);
        let report = import_config(&db, export, ImportMode::Merge, &available).await.unwrap();

        let reason = report.folders[0].disabled_reason.expect("expected a disabled reason");
        assert!(reason.path_missing);
        assert!(reason.printer_missing);
    }

    #[tokio::test]
    async fn a_folder_exported_as_disabled_by_choice_has_no_reason_when_everything_checks_out() {
        let db = connect("sqlite::memory:").await.unwrap();
        let watched = std::env::temp_dir().join("printy-import-disabled-by-choice");
        std::fs::create_dir_all(&watched).unwrap();
        let mut folder = exported_folder("Paused", watched.to_str().unwrap(), "Brother");
        folder.enabled = false;
        let export = export_with(vec![folder]);

        let available = printers(&["Brother"]);
        let report = import_config(&db, export, ImportMode::Merge, &available).await.unwrap();

        assert_eq!(report.disabled, 0, "not a safety-net disable, so it must not count as one");
        let outcome = &report.folders[0];
        assert!(!outcome.enabled);
        assert!(outcome.disabled_reason.is_none());

        std::fs::remove_dir_all(&watched).ok();
    }

    /// `schema_version` rejection happens earlier, in
    /// `schema::parse_export` (covered in `config::schema::tests`), before an
    /// `ImportMode` is even chosen. This just confirms the empty-input edge
    /// case `import_config` itself must handle cleanly: zero totals, no
    /// settings skipped.
    #[tokio::test]
    async fn importing_an_empty_folder_list_is_a_no_op_with_zero_folder_totals() {
        let db = connect("sqlite::memory:").await.unwrap();
        let export = export_with(vec![]);
        let available = printers(&[]);
        let report = import_config(&db, export, ImportMode::Merge, &available).await.unwrap();
        assert_eq!(report.total_folders, 0);
        assert_eq!(report.created, 0);
        assert_eq!(report.updated, 0);
        assert_eq!(report.disabled, 0);
        assert_eq!(report.settings_applied, 4);
    }
}
