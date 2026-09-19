use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Mojang's global version manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionSummary {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: VersionKind,
    pub url: String,
    pub time: String,
    pub release_time: String,
    pub sha1: String,
    #[serde(default)]
    pub compliance_level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionKind {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for VersionKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Release => formatter.write_str("release"),
            Self::Snapshot => formatter.write_str("snapshot"),
            Self::OldBeta => formatter.write_str("old beta"),
            Self::OldAlpha => formatter.write_str("old alpha"),
            Self::Unknown => formatter.write_str("unknown"),
        }
    }
}

/// Per-version metadata. Optional/default fields preserve compatibility across eras.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionMetadata {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: VersionKind,
    pub main_class: String,
    #[serde(default)]
    pub arguments: Option<ModernArguments>,
    #[serde(default)]
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub asset_index: Option<AssetIndexReference>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default)]
    pub downloads: HashMap<String, ArtifactDownload>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub java_version: Option<JavaVersionRequirement>,
    #[serde(default)]
    pub logging: Option<LoggingConfiguration>,
    #[serde(default)]
    pub inherits_from: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModernArguments {
    #[serde(default)]
    pub game: Vec<ArgumentValue>,
    #[serde(default)]
    pub jvm: Vec<ArgumentValue>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ArgumentValue {
    Plain(String),
    Conditional {
        rules: Vec<Rule>,
        value: StringOrList,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum StringOrList {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDownload {
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexReference {
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
    #[serde(default)]
    pub total_size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub natives: HashMap<String, String>,
    #[serde(default)]
    pub extract: Option<ExtractRules>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<ArtifactDownload>,
    #[serde(default)]
    pub classifiers: HashMap<String, ArtifactDownload>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractRules {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JavaVersionRequirement {
    pub component: String,
    #[serde(rename = "majorVersion")]
    pub major_version: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfiguration {
    #[serde(default)]
    pub client: Option<ClientLoggingConfiguration>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClientLoggingConfiguration {
    pub argument: String,
    pub file: LoggingFile,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingFile {
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: HashMap<String, bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}
