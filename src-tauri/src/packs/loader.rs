//! Reading a pack directory into something the runtime can spawn, and managing
//! the packs directory: install, uninstall, scan, and `.petpack` extraction.
use super::{
    diagnostics::{Diagnostic, Report, Tier},
    limits::*,
    manifest::{self, Manifest, Version, MANIFEST_FILE},
    sprites::{self, Strip},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Pack {
    pub manifest: Manifest,
    pub directory: PathBuf,
    /// Decoded strips keyed by the clip name the manifest declared.
    pub strips: BTreeMap<String, Strip>,
    pub preview: Option<String>,
    pub report: Report,
    pub builtin: bool,
}

impl Pack {
    pub fn strip(&self, clip: &str) -> Option<&Strip> {
        self.strips.get(self.manifest.resolve_clip(clip))
    }
    pub fn summary(&self) -> PackSummary {
        let m = &self.manifest;
        PackSummary {
            id: m.id.clone(),
            name: m.name.clone(),
            author: m.author.clone(),
            version: m.version.clone(),
            description: m.description.clone(),
            license: m.license.clone(),
            tags: m.tags.clone(),
            class: m.locomotion.class.as_str().to_string(),
            surfaces: m
                .locomotion
                .surfaces
                .iter()
                .map(|s| s.as_str().to_string())
                .collect(),
            animations: m.animations.keys().cloned().collect(),
            assistant_clips: manifest::ASSISTANT_CLIPS
                .iter()
                .map(|(slot, when)| AssistantClip {
                    slot: (*slot).into(),
                    when: (*when).into(),
                    clip: m.resolve_clip(slot).to_string(),
                    provided: m.animations.contains_key(*slot),
                })
                .collect(),
            preview: self.preview.clone(),
            directory: self.directory.to_string_lossy().to_string(),
            builtin: self.builtin,
            errors: self
                .report
                .diagnostics
                .iter()
                .filter(|d| d.tier == Tier::Error)
                .count(),
            warnings: self
                .report
                .diagnostics
                .iter()
                .filter(|d| d.tier == Tier::Warning)
                .count(),
            notes: self
                .report
                .diagnostics
                .iter()
                .filter(|d| d.tier == Tier::Info)
                .count(),
            loadable: true,
        }
    }
    /// Sprite atlas for the renderer: one data URI and geometry per clip.
    pub fn atlas(&self) -> BTreeMap<String, ClipAtlas> {
        self.strips
            .iter()
            .filter_map(|(name, strip)| {
                let animation = self.manifest.animations.get(name)?;
                Some((
                    name.clone(),
                    ClipAtlas {
                        image: strip.data_uri.clone(),
                        frames: animation.frames,
                        fps: animation.fps,
                        looping: animation.looping,
                        pingpong: animation.pingpong,
                        frame_width: strip.frame_width,
                        height: strip.height,
                    },
                ))
            })
            .collect()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipAtlas {
    pub image: String,
    pub frames: u32,
    pub fps: f64,
    pub looping: bool,
    pub pingpong: bool,
    pub frame_width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackSummary {
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub license: String,
    pub tags: Vec<String>,
    pub class: String,
    pub surfaces: Vec<String>,
    pub animations: Vec<String>,
    pub assistant_clips: Vec<AssistantClip>,
    pub preview: Option<String>,
    pub directory: String,
    pub builtin: bool,
    pub errors: usize,
    pub warnings: usize,
    /// Info diagnostics: nothing is wrong, but the author may want to know —
    /// an empty optional clip slot, for instance.
    pub notes: usize,
    pub loadable: bool,
}

/// One assistant clip slot as this pack actually resolves it. The packs UI
/// shows these so an author can see, without reading the fallback chain, what
/// their pet will play while it is working or waiting on the user.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantClip {
    pub slot: String,
    /// When the slot plays, in words.
    pub when: String,
    /// The clip actually played, which is the slot itself when provided.
    pub clip: String,
    pub provided: bool,
}

/// A pack that failed to load still has to appear in the browser, with reasons.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackListing {
    pub summary: PackSummary,
    pub report: Report,
}

pub fn load(directory: &Path, builtin: bool) -> Result<Pack, Report> {
    let mut report = Report::default();
    let manifest_path = directory.join(MANIFEST_FILE);
    if std::fs::metadata(&manifest_path).is_ok_and(|m| m.len() > MAX_MANIFEST_BYTES) {
        report.push(Diagnostic::error(
            MANIFEST_FILE,
            "",
            "The manifest is unreasonably large.",
            format!(
                "Keep {MANIFEST_FILE} under {} KiB.",
                MAX_MANIFEST_BYTES >> 10
            ),
        ));
        return Err(report);
    }
    let text = match std::fs::read_to_string(&manifest_path) {
        Ok(text) => text,
        Err(error) => {
            report.push(Diagnostic::error(
                MANIFEST_FILE,
                "",
                format!("The manifest could not be read: {error}."),
                format!("Every pack needs a `{MANIFEST_FILE}` beside its `sprites` folder."),
            ));
            return Err(report);
        }
    };
    let Some(mut parsed) = manifest::parse(&text, &mut report) else {
        return Err(report);
    };
    if parsed.reactions.is_empty() {
        parsed.reactions = manifest::default_reactions(&parsed.behavior);
    }
    let mut strips = BTreeMap::new();
    for (name, animation) in &parsed.animations {
        let Some(path) = safe_join(directory, &animation.file) else {
            report.push(Diagnostic::error(
                MANIFEST_FILE,
                format!("animations.{name}.file"),
                format!("`{}` points outside the pack folder.", animation.file),
                "Sprite paths are relative to the pack, e.g. `sprites/idle.png`.",
            ));
            continue;
        };
        if let Some(strip) = sprites::load(&path, &animation.file, animation.frames, &mut report) {
            check_geometry(&parsed, name, animation, &strip, &mut report);
            strips.insert(name.clone(), strip);
        }
    }
    if !directory.join("LICENSE.txt").exists() {
        report.push(Diagnostic::warning(
            "LICENSE.txt",
            "",
            "The pack has no licence file.",
            "Add LICENSE.txt before distributing so people know what they may do with the art.",
        ));
    }
    let preview = load_preview(directory, &mut report);
    for stray in stray_files(directory) {
        report.push(Diagnostic::warning(
            stray.clone(),
            "",
            "This file is not part of the pack format.",
            "Packs hold `pet.json`, `preview.png`, `LICENSE.txt` and `sprites/*.png`; anything else is dropped when the pack is exported.",
        ));
    }
    if report.has_errors() {
        return Err(report);
    }
    Ok(Pack {
        manifest: parsed,
        directory: directory.to_path_buf(),
        strips,
        preview,
        report,
        builtin,
    })
}

fn load_preview(directory: &Path, report: &mut Report) -> Option<String> {
    let path = directory.join("preview.png");
    if !path.exists() {
        report.push(Diagnostic::warning(
            "preview.png",
            "",
            "The pack has no preview image.",
            "Add a 256×256 preview.png so the pack browser can show it.",
        ));
        return None;
    }
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_ENTRY_BYTES) {
        report.push(Diagnostic::error(
            "preview.png",
            "",
            "The preview image is too large.",
            format!(
                "Keep it under {} MiB; 256×256 is plenty.",
                MAX_ENTRY_BYTES >> 20
            ),
        ));
        return None;
    }
    std::fs::read(&path)
        .ok()
        .map(|bytes| format!("data:image/png;base64,{}", sprites::base64(&bytes)))
}

/// The only files a pack may hold. Everything else is stray: ignored when
/// loading a folder, refused in an archive, and left out of an export.
pub fn is_allowed_path(relative: &str) -> bool {
    match relative {
        MANIFEST_FILE | "preview.png" | "LICENSE.txt" => true,
        _ => relative.strip_prefix("sprites/").is_some_and(|file| {
            file.strip_suffix(".png").is_some_and(|stem| {
                !stem.is_empty()
                    && stem.chars().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'
                    })
            })
        }),
    }
}

/// Every file under `directory` (as `/`-separated relative paths, sorted).
fn pack_files(directory: &Path) -> Vec<String> {
    fn walk(root: &Path, at: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(at) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                walk(root, &path, out);
            } else if let Ok(relative) = path.strip_prefix(root) {
                out.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut files = Vec::new();
    walk(directory, directory, &mut files);
    files.sort();
    files
}

fn stray_files(directory: &Path) -> Vec<String> {
    pack_files(directory)
        .into_iter()
        .filter(|f| !is_allowed_path(f))
        .collect()
}

fn check_geometry(
    manifest: &Manifest,
    name: &str,
    animation: &manifest::Animation,
    strip: &Strip,
    report: &mut Report,
) {
    let expected = manifest.sprite.width * animation.frames as f64;
    if (strip.width as f64 - expected).abs() > 0.5 {
        report.push(Diagnostic::warning(
            animation.file.clone(),
            "",
            format!(
                "The image is {} px wide but {} frames of {} px need {} px.",
                strip.width, animation.frames, manifest.sprite.width, expected
            ),
            format!(
                "Each frame will be treated as {} px wide instead.",
                strip.frame_width
            ),
        ));
    }
    if (strip.height as f64 - manifest.sprite.height).abs() > 0.5 {
        report.push(Diagnostic::info(
            animation.file.clone(),
            "",
            format!(
                "Image is {}×{} but sprite.height is {}.",
                strip.width, strip.height, manifest.sprite.height
            ),
            format!(
                "{:.0} px of vertical space is unused.",
                (strip.height as f64 - manifest.sprite.height).abs()
            ),
        ));
    }
    let _ = name;
}

/// Join a pack-relative path, refusing anything that escapes the pack folder.
pub fn safe_join(root: &Path, relative: &str) -> Option<PathBuf> {
    let candidate = Path::new(relative);
    if candidate.is_absolute() {
        return None;
    }
    let mut path = root.to_path_buf();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => path.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(path)
}

/// Every pack directory found, loadable or not.
pub fn scan(packs_dir: &Path) -> Vec<PackListing> {
    let mut listings = Vec::new();
    let Ok(entries) = std::fs::read_dir(packs_dir) else {
        return listings;
    };
    let mut directories: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        // `.staging` and other dot folders are Ledgeling's own scratch space.
        .filter(|p| {
            !p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .collect();
    directories.sort();
    for directory in directories {
        let builtin = super::builtin::is_builtin_directory(&directory);
        match load(&directory, builtin) {
            Ok(pack) => listings.push(PackListing {
                summary: pack.summary(),
                report: pack.report.clone(),
            }),
            Err(report) => listings.push(PackListing {
                summary: broken_summary(&directory, &report, builtin),
                report,
            }),
        }
    }
    listings
}

fn broken_summary(directory: &Path, report: &Report, builtin: bool) -> PackSummary {
    let name = directory
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "pack".into());
    PackSummary {
        id: format!("unloadable:{name}"),
        name: name.clone(),
        author: String::new(),
        version: String::new(),
        description: "This pack could not be loaded.".into(),
        license: String::new(),
        tags: Vec::new(),
        class: String::new(),
        surfaces: Vec::new(),
        animations: Vec::new(),
        assistant_clips: Vec::new(),
        preview: None,
        directory: directory.to_string_lossy().to_string(),
        builtin,
        errors: report
            .diagnostics
            .iter()
            .filter(|d| d.tier == Tier::Error)
            .count(),
        warnings: report
            .diagnostics
            .iter()
            .filter(|d| d.tier == Tier::Warning)
            .count(),
        notes: report
            .diagnostics
            .iter()
            .filter(|d| d.tier == Tier::Info)
            .count(),
        loadable: false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallMode {
    /// The id must not be installed already.
    New,
    /// The id must be installed, at a strictly lower version.
    Update,
}

/// Install a `.petpack` (a renamed zip) or a plain folder into the packs
/// directory. Duplicate ids are rejected with the name of the existing pack.
pub fn install(source: &Path, packs_dir: &Path) -> Result<PackSummary, String> {
    install_with(source, packs_dir, InstallMode::New)
}

pub fn install_with(
    source: &Path,
    packs_dir: &Path,
    mode: InstallMode,
) -> Result<PackSummary, String> {
    let staging = packs_dir.join(".staging");
    let staged = if source.is_dir() {
        source.to_path_buf()
    } else {
        extract(source, &staging)?
    };
    let result = install_staged(&staged, packs_dir, mode);
    let _ = std::fs::remove_dir_all(&staging);
    result
}

fn install_staged(
    staged: &Path,
    packs_dir: &Path,
    mode: InstallMode,
) -> Result<PackSummary, String> {
    let pack = load(staged, false)
        .map_err(|report| format!("This pack cannot be installed yet:\n\n{}", report.text()))?;
    let existing = scan(packs_dir)
        .into_iter()
        .find(|e| e.summary.id == pack.manifest.id);
    match (&existing, mode) {
        (Some(existing), InstallMode::New) => {
            return Err(format!(
                "A pack with the id `{}` is already installed: {} by {}. \
Change the `id` in pet.json, or update the installed pack instead.",
                pack.manifest.id, existing.summary.name, existing.summary.author
            ));
        }
        (None, InstallMode::Update) => {
            return Err(format!(
                "No installed pack has the id `{}`, so there is nothing to update.",
                pack.manifest.id
            ));
        }
        (Some(existing), InstallMode::Update) => {
            if existing.summary.builtin {
                return Err("Bundled pets cannot be replaced.".into());
            }
            let (old, new) = (
                Version::parse(&existing.summary.version),
                Version::parse(&pack.manifest.version),
            );
            match (old, new) {
                (Some(old), Some(new)) if new > old => {}
                _ => {
                    return Err(format!(
                        "`{}` is installed at version {}; an update must have a higher \
three-number version, but this one is {}.",
                        pack.manifest.id, existing.summary.version, pack.manifest.version
                    ));
                }
            }
        }
        (None, InstallMode::New) => {}
    }
    let target = match &existing {
        Some(existing) => PathBuf::from(&existing.summary.directory),
        None => packs_dir.join(folder_name(&pack.manifest.id)),
    };
    let folder = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| folder_name(&pack.manifest.id));
    // Build the new copy beside the target, then swap: a failed copy leaves
    // whatever was installed untouched.
    let incoming = packs_dir.join(format!(".incoming-{folder}"));
    let backup = packs_dir.join(format!(".old-{folder}"));
    let _ = std::fs::remove_dir_all(&incoming);
    let _ = std::fs::remove_dir_all(&backup);
    if let Err(e) = copy_pack(staged, &incoming) {
        let _ = std::fs::remove_dir_all(&incoming);
        return Err(format!("Could not copy the pack into place: {e}"));
    }
    if target.exists() {
        if let Err(e) = std::fs::rename(&target, &backup) {
            let _ = std::fs::remove_dir_all(&incoming);
            return Err(format!("Could not replace {target:?}: {e}"));
        }
        if let Err(e) = std::fs::rename(&incoming, &target) {
            let _ = std::fs::rename(&backup, &target);
            let _ = std::fs::remove_dir_all(&incoming);
            return Err(format!("Could not move the pack into place: {e}"));
        }
        let _ = std::fs::remove_dir_all(&backup);
    } else if let Err(e) = std::fs::rename(&incoming, &target) {
        let _ = std::fs::remove_dir_all(&incoming);
        return Err(format!("Could not move the pack into place: {e}"));
    }
    load(&target, false)
        .map(|pack| pack.summary())
        .map_err(|report| report.text())
}

pub fn uninstall(packs_dir: &Path, id: &str) -> Result<(), String> {
    for listing in scan(packs_dir) {
        if listing.summary.id == id {
            return std::fs::remove_dir_all(&listing.summary.directory)
                .map_err(|e| format!("Could not remove the pack: {e}"));
        }
    }
    Err(format!("No installed pack has the id `{id}`."))
}

pub(crate) fn folder_name(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Archive entries that are OS litter rather than pack content.
fn is_archive_litter(name: &str) -> bool {
    name.starts_with("__MACOSX/")
        || name == ".DS_Store"
        || name.ends_with("/.DS_Store")
        || name.ends_with("/Thumbs.db")
        || name == "Thumbs.db"
}

/// Extract an archive into `into`, refusing anything a pack may not contain.
/// Declared sizes in a zip can lie, so every read is bounded as well as checked.
fn extract(archive: &Path, into: &Path) -> Result<PathBuf, String> {
    let _ = std::fs::remove_dir_all(into);
    std::fs::create_dir_all(into)
        .map_err(|e| format!("Could not prepare the staging folder: {e}"))?;
    let size = std::fs::metadata(archive)
        .map_err(|e| format!("Could not open {}: {e}", archive.display()))?
        .len();
    if size > MAX_ARCHIVE_BYTES {
        return Err(format!(
            "{} is {} MiB; a .petpack may be at most {} MiB.",
            archive.display(),
            size >> 20,
            MAX_ARCHIVE_BYTES >> 20
        ));
    }
    let file = std::fs::File::open(archive)
        .map_err(|e| format!("Could not open {}: {e}", archive.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| {
        format!(
            "{} is not a readable .petpack archive: {e}",
            archive.display()
        )
    })?;
    if zip.len() > MAX_ARCHIVE_ENTRIES {
        return Err(format!(
            "The archive holds {} entries; a pack may hold at most {MAX_ARCHIVE_ENTRIES}.",
            zip.len()
        ));
    }
    // Archives are often zipped with the pack folder inside; find that prefix.
    let names: Vec<String> = zip.file_names().map(|n| n.replace('\\', "/")).collect();
    let prefix = if names.iter().any(|n| n == MANIFEST_FILE) {
        String::new()
    } else {
        let wrapped: Vec<&String> = names
            .iter()
            .filter(|n| {
                n.strip_suffix(MANIFEST_FILE)
                    .is_some_and(|p| p.matches('/').count() == 1 && p.ends_with('/'))
            })
            .collect();
        match wrapped.as_slice() {
            [only] => only.trim_end_matches(MANIFEST_FILE).to_string(),
            _ => {
                return Err(format!(
                    "The archive has no {MANIFEST_FILE} at its top level. \
A .petpack is a zip of the pack folder's contents."
                ))
            }
        }
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut total: u64 = 0;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| format!("Could not read the archive: {e}"))?;
        let raw = entry.name().replace('\\', "/");
        if is_archive_litter(&raw) {
            continue;
        }
        if entry.enclosed_name().is_none() {
            return Err(format!("`{raw}` points outside the pack folder."));
        }
        if entry.is_dir() {
            continue;
        }
        if entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err(format!(
                "`{raw}` is a symbolic link, which packs may not contain."
            ));
        }
        let Some(name) = raw.strip_prefix(prefix.as_str()) else {
            return Err(format!("`{raw}` is outside the pack folder `{prefix}`."));
        };
        if !is_allowed_path(name) {
            return Err(format!(
                "`{name}` is not allowed in a pack. Packs hold `{MANIFEST_FILE}`, `preview.png`, \
`LICENSE.txt` and `sprites/*.png` (lowercase letters, digits, `-` and `_`)."
            ));
        }
        if !seen.insert(name.to_string()) {
            return Err(format!("`{name}` appears twice in the archive."));
        }
        if entry.size() > MAX_ENTRY_BYTES {
            return Err(format!(
                "`{name}` is {} MiB uncompressed; a pack file may be at most {} MiB.",
                entry.size() >> 20,
                MAX_ENTRY_BYTES >> 20
            ));
        }
        let mut bytes = Vec::new();
        (&mut entry)
            .take(MAX_ENTRY_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Could not read {name}: {e}"))?;
        if bytes.len() as u64 > MAX_ENTRY_BYTES {
            return Err(format!(
                "`{name}` is larger than the {} MiB limit for one pack file.",
                MAX_ENTRY_BYTES >> 20
            ));
        }
        total += bytes.len() as u64;
        if total > MAX_UNCOMPRESSED_TOTAL {
            return Err(format!(
                "The archive expands to more than {} MiB, which is the limit for a pack.",
                MAX_UNCOMPRESSED_TOTAL >> 20
            ));
        }
        let target = into.join(name);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
        }
        std::fs::write(&target, bytes).map_err(|e| format!("{e}"))?;
    }
    Ok(into.to_path_buf())
}

/// Copy only the files a pack may hold.
fn copy_pack(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for relative in pack_files(from) {
        if !is_allowed_path(&relative) {
            continue;
        }
        let target = to.join(&relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(from.join(&relative), target)?;
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Exported {
    pub report: Report,
    /// Lowercase hex SHA-256 of the `.petpack` bytes.
    pub sha256: String,
}

/// Zip a pack folder into a canonical `.petpack`: allow-listed files only, in
/// sorted order, with fixed timestamps and permissions, so the same pack always
/// produces the same bytes (and the same hash).
pub fn export(directory: &Path, destination: &Path) -> Result<Exported, String> {
    let pack = load(directory, false).map_err(|report| report.text())?;
    let files: Vec<String> = pack_files(directory)
        .into_iter()
        .filter(|f| is_allowed_path(f))
        .collect();
    if files.len() > MAX_ARCHIVE_ENTRIES {
        return Err(format!(
            "The pack has {} files; a pack may hold at most {MAX_ARCHIVE_ENTRIES}.",
            files.len()
        ));
    }
    let options: zip::write::FileOptions<()> = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let mut total: u64 = 0;
    for relative in &files {
        let bytes = std::fs::read(directory.join(relative))
            .map_err(|e| format!("Could not read {relative}: {e}"))?;
        total += bytes.len() as u64;
        if bytes.len() as u64 > MAX_ENTRY_BYTES || total > MAX_UNCOMPRESSED_TOTAL {
            return Err(format!(
                "`{relative}` makes the pack larger than the size limit."
            ));
        }
        zip.start_file(relative.as_str(), options)
            .map_err(|e| format!("Could not add {relative}: {e}"))?;
        std::io::Write::write_all(&mut zip, &bytes).map_err(|e| format!("{e}"))?;
    }
    let bytes = zip
        .finish()
        .map_err(|e| format!("Could not finish the archive: {e}"))?
        .into_inner();
    if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
        return Err(format!(
            "The archive is {} MiB; a .petpack may be at most {} MiB.",
            bytes.len() >> 20,
            MAX_ARCHIVE_BYTES >> 20
        ));
    }
    std::fs::write(destination, &bytes)
        .map_err(|e| format!("Could not create {}: {e}", destination.display()))?;
    Ok(Exported {
        report: pack.report,
        sha256: hex(&Sha256::digest(&bytes)),
    })
}

/// SHA-256 of a `.petpack` file, lowercase hex. The same function a client uses
/// to check a download against the hash the gallery published.
pub fn hash_petpack(path: &Path) -> Result<String, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    Ok(hex(&Sha256::digest(&bytes)))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Licences a gallery pack may declare. Ids follow SPDX where one exists.
pub const PUBLISH_LICENSES: &[&str] = &[
    "CC0-1.0",
    "CC-BY-4.0",
    "CC-BY-SA-4.0",
    "CC-BY-NC-4.0",
    "MIT",
    "Apache-2.0",
    "proprietary-redistributable",
];

fn png_size(path: &Path) -> Option<(u32, u32)> {
    let file = std::fs::File::open(path).ok()?;
    let reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .ok()?;
    let info = reader.info();
    Some((info.width, info.height))
}

fn is_reverse_dns(id: &str) -> bool {
    let segments: Vec<&str> = id.split('.').collect();
    segments.len() >= 2
        && segments.iter().all(|s| {
            !s.is_empty()
                && !s.starts_with('-')
                && !s.ends_with('-')
                && s.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
}

/// Everything `load` checks, plus the stricter rules for a pack that strangers
/// will download. Local packs need not meet these; a gallery upload must.
pub fn validate_for_publish(directory: &Path) -> Report {
    let pack = match load(directory, false) {
        Ok(pack) => pack,
        Err(report) => return report,
    };
    let mut report = pack.report.clone();
    let manifest = &pack.manifest;
    let mut fail = |file: &str, path: &str, what: String, expected: String| {
        report.push(Diagnostic::error(file, path, what, expected));
    };
    if Version::parse(&manifest.version).is_none() {
        fail(
            MANIFEST_FILE,
            "version",
            format!("`{}` is not a three-number version.", manifest.version),
            "Use `major.minor.patch`, like `1.0.0`; updates are ordered by it.".into(),
        );
    }
    if !is_reverse_dns(&manifest.id) {
        fail(
            MANIFEST_FILE,
            "id",
            format!("`{}` is not a reverse-DNS identifier.", manifest.id),
            "Use lowercase letters, digits and `-` in dot-separated parts, like `com.yourname.simplebird`.".into(),
        );
    }
    if !PUBLISH_LICENSES.contains(&manifest.license.as_str()) {
        fail(
            MANIFEST_FILE,
            "license",
            format!("`{}` is not a recognised licence.", manifest.license),
            format!("Set `license` to one of: {}.", PUBLISH_LICENSES.join(", ")),
        );
    }
    if !std::fs::metadata(directory.join("LICENSE.txt")).is_ok_and(|m| m.len() > 0) {
        fail(
            "LICENSE.txt",
            "",
            "The pack has no licence text.".into(),
            "Add a non-empty LICENSE.txt; downloaders need to know what they may do with the art."
                .into(),
        );
    }
    if manifest.description.trim().is_empty() {
        fail(
            MANIFEST_FILE,
            "description",
            "The description is empty.".into(),
            "Say in a sentence what the pet is.".into(),
        );
    }
    match png_size(&directory.join("preview.png")) {
        Some((w, h)) if (w, h) == (PREVIEW_SIZE, PREVIEW_SIZE) => {}
        Some((w, h)) => fail(
            "preview.png",
            "",
            format!("The preview is {w}×{h} px."),
            format!("Make it exactly {PREVIEW_SIZE}×{PREVIEW_SIZE} px."),
        ),
        None => fail(
            "preview.png",
            "",
            "The pack has no readable preview.png.".into(),
            format!("Add a {PREVIEW_SIZE}×{PREVIEW_SIZE} px preview.png."),
        ),
    }
    for (field, value) in [
        ("width", manifest.sprite.width),
        ("height", manifest.sprite.height),
    ] {
        if (value - PUBLISH_SPRITE_SIZE as f64).abs() > f64::EPSILON {
            fail(
                MANIFEST_FILE,
                &format!("sprite.{field}"),
                format!("Found {value}."),
                format!(
                    "Published pets use {PUBLISH_SPRITE_SIZE}×{PUBLISH_SPRITE_SIZE} px frames."
                ),
            );
        }
    }
    for (name, animation) in &manifest.animations {
        if let Some(strip) = pack.strips.get(name) {
            let wide = manifest.sprite.width * animation.frames as f64;
            if (strip.width as f64 - wide).abs() > 0.5
                || (strip.height as f64 - manifest.sprite.height).abs() > 0.5
            {
                fail(
                    &animation.file,
                    "",
                    format!(
                        "The strip is {}×{} px but {} frames need {wide}×{} px.",
                        strip.width, strip.height, animation.frames, manifest.sprite.height
                    ),
                    "Make every strip exactly frames × frame width wide and one frame tall.".into(),
                );
            }
        }
    }
    for stray in stray_files(directory) {
        fail(
            &stray,
            "",
            "This file is not part of the pack format.".into(),
            "Remove it; published packs hold only pet.json, preview.png, LICENSE.txt and sprites/*.png.".into(),
        );
    }
    report
}
