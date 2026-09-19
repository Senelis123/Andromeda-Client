use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

use super::VersionManifest;
use crate::{domain::AppError, storage::atomic_write};

const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const CACHE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogSource {
    Network,
    Cache,
    NotModified,
}

#[derive(Debug, Clone)]
pub struct CatalogUpdate {
    pub manifest: VersionManifest,
    pub source: CatalogSource,
    pub warning: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VersionCatalogService {
    client: Client,
    cache_path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct CachedManifest {
    schema_version: u32,
    #[serde(default)]
    etag: Option<String>,
    manifest: VersionManifest,
}

impl VersionCatalogService {
    pub fn new(cache_root: &Path) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .user_agent(concat!("Andromeda-Client/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;
        Ok(Self {
            client,
            cache_path: cache_root.join("metadata/version_manifest_v2.json"),
        })
    }

    pub async fn refresh(&self) -> Result<CatalogUpdate, String> {
        let cached = self.load_cache().map_err(|error| error.to_string())?;
        let mut request = self.client.get(VERSION_MANIFEST_URL);
        if let Some(etag) = cached.as_ref().and_then(|cache| cache.etag.as_ref()) {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        match request.send().await {
            Ok(response) if response.status() == StatusCode::NOT_MODIFIED => cached
                .map(|cache| CatalogUpdate {
                    manifest: cache.manifest,
                    source: CatalogSource::NotModified,
                    warning: None,
                })
                .ok_or_else(|| "Mojang returned not-modified but no valid cache exists".into()),
            Ok(response) if response.status().is_success() => {
                let etag = response
                    .headers()
                    .get(reqwest::header::ETAG)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned);
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|error| format!("could not read Mojang version manifest: {error}"))?;
                if bytes.len() > 8 * 1024 * 1024 {
                    return Err("Mojang version manifest exceeded the 8 MiB safety limit".into());
                }
                let manifest: VersionManifest = serde_json::from_slice(&bytes)
                    .map_err(|error| format!("Mojang version manifest was invalid: {error}"))?;
                validate_manifest(&manifest)?;
                let cache = CachedManifest {
                    schema_version: CACHE_SCHEMA_VERSION,
                    etag,
                    manifest: manifest.clone(),
                };
                let serialized = serde_json::to_vec(&cache).map_err(|error| error.to_string())?;
                atomic_write(&self.cache_path, &serialized).map_err(|error| error.to_string())?;
                Ok(CatalogUpdate {
                    manifest,
                    source: CatalogSource::Network,
                    warning: None,
                })
            }
            Ok(response) => fallback(
                cached,
                format!("Mojang catalog request returned HTTP {}", response.status()),
            ),
            Err(error) => fallback(
                cached,
                format!("Mojang catalog could not be refreshed: {error}"),
            ),
        }
    }

    fn load_cache(&self) -> Result<Option<CachedManifest>, AppError> {
        if !self.cache_path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&self.cache_path).map_err(|source| AppError::FileSystem {
            operation: "read",
            path: self.cache_path.clone(),
            source,
        })?;
        let cache = match serde_json::from_slice::<CachedManifest>(&bytes) {
            Ok(cache)
                if cache.schema_version == CACHE_SCHEMA_VERSION
                    && validate_manifest(&cache.manifest).is_ok() =>
            {
                cache
            }
            _ => return Ok(None),
        };
        Ok(Some(cache))
    }
}

fn fallback(cache: Option<CachedManifest>, warning: String) -> Result<CatalogUpdate, String> {
    match cache {
        Some(cache) => Ok(CatalogUpdate {
            manifest: cache.manifest,
            source: CatalogSource::Cache,
            warning: Some(warning),
        }),
        None => Err(warning),
    }
}

fn validate_manifest(manifest: &VersionManifest) -> Result<(), String> {
    if manifest.versions.is_empty() {
        return Err("Mojang version manifest contained no versions".into());
    }
    if manifest.versions.len() > 100_000 {
        return Err("Mojang version manifest contained too many entries".into());
    }
    for version in &manifest.versions {
        if version.id.is_empty() || version.id.len() > 128 || version.id.contains(['/', '\\', '\0'])
        {
            return Err(format!(
                "version manifest contains an unsafe version ID: {:?}",
                version.id
            ));
        }
        let url = reqwest::Url::parse(&version.url).map_err(|error| {
            format!(
                "version {} has an invalid metadata URL: {error}",
                version.id
            )
        })?;
        if url.scheme() != "https" {
            return Err(format!(
                "version {} metadata does not use HTTPS",
                version.id
            ));
        }
        if version.sha1.len() != 40 || !version.sha1.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("version {} has an invalid SHA-1", version.id));
        }
    }
    Ok(())
}
