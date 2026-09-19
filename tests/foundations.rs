use andromeda_client::{config::{AppSettings, SettingsRepository}, domain::{TaskProgress, TaskState}, telemetry::Secret};

#[test]
fn settings_round_trip() {
    let temp = tempfile::tempdir().unwrap();
    let repository = SettingsRepository::new(temp.path().join("settings.json"));
    let settings = AppSettings { show_snapshots: true, ..Default::default() };
    repository.save(&settings).unwrap();
    let loaded = repository.load().unwrap();
    assert_eq!(loaded.settings, settings);
    assert!(loaded.recovery_notice.is_none());
}

#[test]
fn corrupt_settings_are_preserved_and_recovered() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.json");
    std::fs::write(&path, "not json").unwrap();
    let loaded = SettingsRepository::new(path.clone()).load().unwrap();
    assert_eq!(loaded.settings, AppSettings::default());
    assert!(loaded.recovery_notice.is_some());
    assert!(path.with_extension("corrupt").exists());
}

#[test]
fn secrets_never_format_the_inner_value() {
    let secret = Secret::new("canary-token");
    assert_eq!(format!("{secret}"), "[REDACTED]");
    assert_eq!(format!("{secret:?}"), "[REDACTED]");
}

#[test]
fn progress_is_bounded() {
    let progress = TaskProgress { state: TaskState::Running, completed: 150, total: 100, phase: String::new() };
    assert_eq!(progress.percentage(), 1.0);
}
