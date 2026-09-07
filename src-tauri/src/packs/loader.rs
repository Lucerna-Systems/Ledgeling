//! Reading a pack directory into something the runtime can spawn, and managing
//! the packs directory: install, uninstall, scan, and `.petpack` extraction.
use super::{
    diagnostics::{Diagnostic, Report, Tier},
    manifest::{self, Manifest, MANIFEST_FILE},
    sprites::{self, Strip},
};
use serde::Serialize;
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
    pub preview: Option<String>,
    pub directory: String,
    pub builtin: bool,
    pub errors: usize,
    pub warnings: usize,
    pub loadable: bool,
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
    std::fs::read(&path)
        .ok()
        .map(|bytes| format!("data:image/png;base64,{}", sprites::base64(&bytes)))
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
        // `.staging` and other dot folders are Perch's own scratch space.
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
        loadable: false,
    }
}

/// Install a `.petpack` (a renamed zip) or a plain folder into the packs
/// directory. Duplicate ids are rejected with the name of the existing pack.
pub fn install(source: &Path, packs_dir: &Path) -> Result<PackSummary, String> {
    let staged = if source.is_dir() {
        source.to_path_buf()
    } else {
        extract(source, &packs_dir.join(".staging"))?
    };
    let pack = load(&staged, false)
        .map_err(|report| format!("This pack cannot be installed yet:\n\n{}", report.text()))?;
    for existing in scan(packs_dir) {
        if existing.summary.id == pack.manifest.id {
            let _ = std::fs::remove_dir_all(packs_dir.join(".staging"));
            return Err(format!(
                "A pack with the id `{}` is already installed: {} by {}. \
Change the `id` in pet.json, or remove the installed pack first.",
                pack.manifest.id, existing.summary.name, existing.summary.author
            ));
        }
    }
    let target = packs_dir.join(folder_name(&pack.manifest.id));
    if target.exists() {
        std::fs::remove_dir_all(&target)
            .map_err(|e| format!("Could not replace {target:?}: {e}"))?;
    }
    copy_dir(&staged, &target).map_err(|e| format!("Could not copy the pack into place: {e}"))?;
    let _ = std::fs::remove_dir_all(packs_dir.join(".staging"));
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

fn folder_name(id: &str) -> String {
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

fn extract(archive: &Path, into: &Path) -> Result<PathBuf, String> {
    let _ = std::fs::remove_dir_all(into);
    std::fs::create_dir_all(into)
        .map_err(|e| format!("Could not prepare the staging folder: {e}"))?;
    let file = std::fs::File::open(archive)
        .map_err(|e| format!("Could not open {}: {e}", archive.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| {
        format!(
            "{} is not a readable .petpack archive: {e}",
            archive.display()
        )
    })?;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| format!("Could not read the archive: {e}"))?;
        let Some(name) = entry.enclosed_name() else {
            // A zip entry that escapes its own folder is never legitimate.
            continue;
        };
        let target = into.join(&name);
        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| format!("{e}"))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
        }
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Could not read {}: {e}", name.display()))?;
        std::fs::write(&target, bytes).map_err(|e| format!("{e}"))?;
    }
    // Archives are often zipped with the pack folder inside; unwrap one level.
    let mut entries: Vec<PathBuf> = std::fs::read_dir(into)
        .map_err(|e| format!("{e}"))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            !p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("__"))
        })
        .collect();
    entries.sort();
    if !into.join(MANIFEST_FILE).exists() {
        if let [only] = entries.as_slice() {
            if only.is_dir() {
                return Ok(only.clone());
            }
        }
        return Err(format!(
            "The archive has no {MANIFEST_FILE} at its top level. \
A .petpack is a zip of the pack folder's contents."
        ));
    }
    Ok(into.to_path_buf())
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Zip a pack folder into a `.petpack` beside it, after a final validation pass.
pub fn export(directory: &Path, destination: &Path) -> Result<Report, String> {
    let pack = load(directory, false).map_err(|report| report.text())?;
    let file = std::fs::File::create(destination)
        .map_err(|e| format!("Could not create {}: {e}", destination.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    add_to_zip(&mut zip, directory, directory, &options).map_err(|e| format!("{e}"))?;
    zip.finish()
        .map_err(|e| format!("Could not finish the archive: {e}"))?;
    Ok(pack.report)
}

fn add_to_zip<W: std::io::Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    root: &Path,
    directory: &Path,
    options: &zip::write::FileOptions<()>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let name = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if entry.file_type()?.is_dir() {
            add_to_zip(zip, root, &path, options)?;
        } else {
            zip.start_file(name, *options)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            std::io::Write::write_all(zip, &std::fs::read(&path)?)?;
        }
    }
    Ok(())
}
