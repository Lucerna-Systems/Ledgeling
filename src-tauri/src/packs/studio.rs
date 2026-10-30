//! Pet Studio saves: the in-app editor hands over a manifest and raw pixels, and
//! this writes them as an ordinary pack folder. The result goes through the same
//! loader as any other pack, so the editor can never produce something the
//! validator would not accept from a hand-written pack.
use super::{
    loader::{self, PackSummary},
    manifest::MANIFEST_FILE,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Studio frames are small by design; anything past this is not from the editor.
const MAX_SIDE: u32 = 4096;

/// One image the editor drew: an animation strip or the preview, as RGBA bytes.
#[derive(Debug, Deserialize, Serialize)]
pub struct StudioImage {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// A first save must never land on a pack that is already installed: Studio
/// gives every new design its own id, so a clash means something went wrong,
/// and replacing that pack would change every pet already using it.
pub fn ensure_unused(packs_dir: &Path, manifest: &str) -> Result<(), String> {
    let id = serde_json::from_str::<serde_json::Value>(manifest)
        .ok()
        .and_then(|v| v.get("id").and_then(|id| id.as_str()).map(str::to_owned))
        .ok_or("The design has no id yet. Give it a name and try again.")?;
    match loader::scan(packs_dir)
        .into_iter()
        .find(|l| l.summary.id == id)
    {
        Some(existing) => Err(format!(
            "`{}` already uses that id, and saving would replace it. Rename your design and save again.",
            existing.summary.name
        )),
        None => Ok(()),
    }
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

/// A pack as the editor works on it: its manifest text and every strip decoded.
#[derive(Debug, Serialize)]
pub struct StudioPack {
    pub manifest: String,
    pub strips: Vec<StudioImage>,
}

/// Studio draws 48 x 48 frames; other sizes are edited as files.
const STUDIO_FRAME: u64 = 48;

/// Read a pack folder into the editor. Refuses, in plain words, anything the
/// editor could not draw back out faithfully.
pub fn open(directory: &Path) -> Result<StudioPack, String> {
    let manifest = std::fs::read_to_string(directory.join(MANIFEST_FILE))
        .map_err(|e| format!("Could not read {MANIFEST_FILE}: {e}"))?;
    let value: serde_json::Value = serde_json::from_str(&manifest)
        .map_err(|e| format!("{MANIFEST_FILE} is not readable JSON: {e}"))?;
    let side = |key: &str| {
        value
            .pointer(&format!("/sprite/{key}"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    };
    let (width, height) = (side("width"), side("height"));
    if (width, height) != (STUDIO_FRAME, STUDIO_FRAME) {
        return Err(format!(
            "Studio edits {STUDIO_FRAME}×{STUDIO_FRAME} pets; this one is {width}×{height}. Edit its files directly."
        ));
    }
    let mut strips = Vec::new();
    let animations = value
        .get("animations")
        .and_then(|a| a.as_object())
        .ok_or("This pet has no animations to edit.")?;
    for (name, animation) in animations {
        let file = animation
            .get("file")
            .and_then(|f| f.as_str())
            .ok_or_else(|| format!("The `{name}` animation names no file."))?;
        if file.contains("..") || Path::new(file).is_absolute() {
            return Err(format!(
                "The `{name}` animation points outside the pet's folder."
            ));
        }
        strips.push(read_strip(name, &directory.join(file))?);
    }
    Ok(StudioPack { manifest, strips })
}

fn read_strip(name: &str, path: &Path) -> Result<StudioImage, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("Could not read the `{name}` sprite {}: {e}", path.display()))?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(&bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("The `{name}` sprite is not a readable PNG: {e}"))?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|e| format!("The `{name}` sprite could not be decoded: {e}"))?;
    if info.width == 0 || info.height == 0 || info.width > MAX_SIDE || info.height > MAX_SIDE {
        return Err(format!("The `{name}` sprite has an impossible size."));
    }
    let data = &buffer[..info.buffer_size()];
    let pixels: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => data
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => {
            return Err(format!(
                "The `{name}` sprite uses a palette Studio cannot read."
            ))
        }
    };
    Ok(StudioImage {
        name: name.to_string(),
        width: info.width,
        height: info.height,
        pixels,
    })
}
