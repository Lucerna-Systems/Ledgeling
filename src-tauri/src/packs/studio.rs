//! Pet Studio saves: the in-app editor hands over a manifest and raw pixels, and
//! this writes them as an ordinary pack folder. The result goes through the same
//! loader as any other pack, so the editor can never produce something the
//! validator would not accept from a hand-written pack.
use super::{
    loader::{self, PackSummary},
    manifest::MANIFEST_FILE,
};
use serde::Deserialize;
use std::path::Path;

/// Studio frames are small by design; anything past this is not from the editor.
const MAX_SIDE: u32 = 4096;

/// One image the editor drew: an animation strip or the preview, as RGBA bytes.
#[derive(Debug, Deserialize)]
pub struct StudioImage {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Write the pack into `packs_dir`, replacing an earlier Studio save of the same
/// pack. Built-in packs are never overwritten.
pub fn save(
    packs_dir: &Path,
    manifest: &str,
    strips: &[StudioImage],
    preview: &StudioImage,
) -> Result<PackSummary, String> {
    let id = serde_json::from_str::<serde_json::Value>(manifest)
        .ok()
        .and_then(|v| v.get("id").and_then(|id| id.as_str()).map(str::to_owned))
        .ok_or("The pet has no id yet. Give it a name and try again.")?;
    for strip in strips {
        check_image(strip, true)?;
    }
    check_image(preview, false)?;

    let staging = packs_dir.join(".studio-staging");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(staging.join("sprites"))
        .map_err(|e| format!("Could not prepare the pet's folder: {e}"))?;
    let written = write_all(&staging, manifest, strips, preview);
    let result = written.and_then(|()| {
        loader::load(&staging, false)
            .map_err(|report| format!("The pet could not be saved yet:\n\n{}", report.text()))
    });
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }

    for existing in loader::scan(packs_dir) {
        if existing.summary.id == id && existing.summary.builtin {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!(
                "`{}` comes with Ledgeling and cannot be replaced. Rename your pet to save it as a new one.",
                existing.summary.name
            ));
        }
        if existing.summary.id == id {
            std::fs::remove_dir_all(&existing.summary.directory)
                .map_err(|e| format!("Could not replace the earlier save: {e}"))?;
        }
    }
    let target = packs_dir.join(loader::folder_name(&id));
    if target.exists() {
        std::fs::remove_dir_all(&target)
            .map_err(|e| format!("Could not replace {}: {e}", target.display()))?;
    }
    std::fs::rename(&staging, &target)
        .map_err(|e| format!("Could not move the pet into place: {e}"))?;
    loader::load(&target, false)
        .map(|pack| pack.summary())
        .map_err(|report| report.text())
}

fn check_image(image: &StudioImage, strip: bool) -> Result<(), String> {
    let valid_name = !image.name.is_empty()
        && image.name.len() <= 32
        && image
            .name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '_');
    if strip && !valid_name {
        return Err(format!(
            "`{}` is not an animation name Ledgeling knows.",
            image.name
        ));
    }
    if image.width == 0 || image.height == 0 || image.width > MAX_SIDE || image.height > MAX_SIDE {
        return Err(format!(
            "The `{}` image has an impossible size.",
            image.name
        ));
    }
    if image.pixels.len() != (image.width * image.height * 4) as usize {
        return Err(format!("The `{}` image is missing pixels.", image.name));
    }
    Ok(())
}

fn write_all(
    directory: &Path,
    manifest: &str,
    strips: &[StudioImage],
    preview: &StudioImage,
) -> Result<(), String> {
    std::fs::write(directory.join(MANIFEST_FILE), manifest)
        .map_err(|e| format!("Could not write {MANIFEST_FILE}: {e}"))?;
    for strip in strips {
        super::scaffold::write_png(
            &directory.join(format!("sprites/{}.png", strip.name)),
            &strip.pixels,
            strip.width,
            strip.height,
        )?;
    }
    super::scaffold::write_png(
        &directory.join("preview.png"),
        &preview.pixels,
        preview.width,
        preview.height,
    )?;
    std::fs::write(
        directory.join("LICENSE.txt"),
        "Made in Ledgeling's Pet Studio. Replace this with the licence for your art before sharing the pack.\n",
    )
    .map_err(|e| format!("Could not write LICENSE.txt: {e}"))
}
