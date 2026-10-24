//! Installing a pet someone published: download through the gated `net`
//! client, check the published hash, then hand the file to the same install
//! path as any other `.petpack`. This file holds no network code of its own.
use super::{
    limits::MAX_ARCHIVE_BYTES,
    loader::{self, InstallError, InstallMode, PackSummary},
};
use crate::net::{Net, NetError, Scope};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

pub const SOURCES_FILE: &str = ".sources.json";

/// Where an installed pack came from, so the app can later ask "is there a
/// newer one?" without guessing from the id. Kept beside the packs, never in
/// them, so it cannot leak into an export.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    /// The marketplace's own identifier for the pack; opaque to Ledgeling.
    pub gallery_id: String,
    pub version: String,
    pub sha256: String,
}

impl From<NetError> for InstallError {
    fn from(error: NetError) -> InstallError {
        InstallError::new(error.code, error.message)
    }
}

pub fn sources(packs_dir: &Path) -> BTreeMap<String, Source> {
    std::fs::read_to_string(packs_dir.join(SOURCES_FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_sources(packs_dir: &Path, all: &BTreeMap<String, Source>) -> Result<(), InstallError> {
    let text =
        serde_json::to_string_pretty(all).map_err(|e| InstallError::new("io", e.to_string()))?;
    std::fs::write(packs_dir.join(SOURCES_FILE), text).map_err(|e| {
        InstallError::new(
            "io",
            format!("Could not record where the pet came from: {e}"),
        )
    })
}

pub fn record_source(packs_dir: &Path, id: &str, source: Source) -> Result<(), InstallError> {
    let mut all = sources(packs_dir);
    all.insert(id.to_string(), source);
    write_sources(packs_dir, &all)
}

pub fn forget_source(packs_dir: &Path, id: &str) {
    let mut all = sources(packs_dir);
    if all.remove(id).is_some() {
        let _ = write_sources(packs_dir, &all);
    }
}

/// Fetch `url` from the marketplace, check it against `expected_sha256`, and
/// install it. Nothing is unpacked unless the hash matches.
pub fn download_and_install(
    net: &Net,
    packs_dir: &Path,
    url: &str,
    expected_sha256: &str,
    gallery_id: &str,
    mode: InstallMode,
) -> Result<PackSummary, InstallError> {
    let response = net.get(Scope::Marketplace, url, MAX_ARCHIVE_BYTES)?;
    if !response.ok() {
        return Err(InstallError::new(
            "http_status",
            format!("The marketplace answered with status {}.", response.status),
        ));
    }
    std::fs::create_dir_all(packs_dir).map_err(|e| InstallError::new("io", e.to_string()))?;
    let file = packs_dir.join(".download.petpack");
    std::fs::write(&file, &response.body).map_err(|e| InstallError::new("io", e.to_string()))?;
    let result = loader::verify_hash(&file, expected_sha256)
        .and_then(|_| loader::install_with(&file, packs_dir, mode));
    let _ = std::fs::remove_file(&file);
    let summary = result?;
    record_source(
        packs_dir,
        &summary.id,
        Source {
            gallery_id: gallery_id.to_string(),
            version: summary.version.clone(),
            sha256: expected_sha256.trim().to_ascii_lowercase(),
        },
    )?;
    Ok(summary)
}
