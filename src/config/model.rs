use serde::{Deserialize, Serialize};

pub const SETTINGS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference { Light, Dark, #[default] System }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppSettings {
    pub schema_version: u32,
    pub theme: ThemePreference,
    pub download_concurrency: u8,
    pub show_snapshots: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { schema_version: SETTINGS_SCHEMA_VERSION, theme: ThemePreference::System, download_concurrency: 8, show_snapshots: false }
    }
}

impl AppSettings {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != SETTINGS_SCHEMA_VERSION { return Err("unsupported settings schema"); }
        if !(1..=32).contains(&self.download_concurrency) { return Err("download concurrency must be between 1 and 32"); }
        Ok(())
    }
}
