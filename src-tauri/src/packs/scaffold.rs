//! "New Pack": a folder that runs immediately, so an author starts by editing
//! something that already works rather than by staring at an empty directory.
use super::{
    builtin::SCHEMA,
    manifest::{LocomotionClass, MANIFEST_FILE},
};
use std::path::{Path, PathBuf};

const FRAME: u32 = 48;

pub fn create(packs_dir: &Path, class: LocomotionClass, name: &str) -> Result<PathBuf, String> {
    let slug = slugify(name);
    let directory = packs_dir.join(&slug);
    if directory.exists() {
        return Err(format!(
            "A folder called `{slug}` already exists in the packs directory. Pick another name."
        ));
    }
    std::fs::create_dir_all(directory.join("sprites"))
        .map_err(|e| format!("Could not create the pack folder: {e}"))?;
    // `busy` and `attention` are in every scaffold so an author sees the slots
    // exist at all: a pet with a task of its own plays them, and nothing in the
    // manifest would otherwise hint that they are there to fill.
    let clips: &[(&str, u32)] = match class {
        LocomotionClass::Flier => &[
            ("idle", 4),
            ("move", 4),
            ("land", 2),
            ("busy", 4),
            ("attention", 4),
        ],
        LocomotionClass::Climber => &[
            ("idle", 4),
            ("move", 4),
            ("climb", 4),
            ("fall", 2),
            ("busy", 4),
            ("attention", 4),
        ],
        LocomotionClass::Walker => &[("idle", 4), ("move", 4), ("busy", 4), ("attention", 4)],
        LocomotionClass::Hopper => &[
            ("idle", 4),
            ("move", 4),
            ("jump", 4),
            ("busy", 4),
            ("attention", 4),
        ],
    };
    for (clip, frames) in clips {
        write_png(
            &directory.join(format!("sprites/{clip}.png")),
            &placeholder(*frames, clip),
            FRAME * frames,
            FRAME,
        )?;
    }
    write_png(&directory.join("preview.png"), &preview_pixels(), 256, 256)?;
    std::fs::write(
        directory.join(MANIFEST_FILE),
        manifest_text(&slug, name, class, clips),
    )
    .map_err(|e| format!("Could not write {MANIFEST_FILE}: {e}"))?;
    std::fs::write(directory.join("pet-pack-v1.schema.json"), SCHEMA)
        .map_err(|e| format!("Could not write the schema: {e}"))?;
    std::fs::write(directory.join("README.md"), readme_text(name, class))
        .map_err(|e| format!("Could not write README.md: {e}"))?;
    std::fs::write(
        directory.join("LICENSE.txt"),
        "Replace this with the licence for your art before sharing the pack.\n\
         If you are unsure, CC-BY-4.0 lets people use it with credit.\n",
    )
    .map_err(|e| format!("Could not write LICENSE.txt: {e}"))?;
    Ok(directory)
}

fn slugify(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-').replace("--", "-");
    if slug.is_empty() {
        "new-pet".into()
    } else {
        slug
    }
}

fn manifest_text(slug: &str, name: &str, class: LocomotionClass, clips: &[(&str, u32)]) -> String {
    let animations = clips
        .iter()
        .map(|(clip, frames)| {
            format!(
                "    \"{clip}\": {{ \"file\": \"sprites/{clip}.png\", \"frames\": {frames}, \"fps\": {} }}",
                if *clip == "idle" { 6 } else { 10 }
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let surfaces = match class {
        LocomotionClass::Flier => "[\"window_top\", \"monitor_top\", \"taskbar_top\"]",
        LocomotionClass::Climber => "[\"window_any\", \"monitor_any\", \"taskbar_top\"]",
        LocomotionClass::Walker => "[\"window_top\", \"taskbar_top\"]",
        LocomotionClass::Hopper => "[\"window_top\", \"taskbar_top\"]",
    };
    let extra = match class {
        LocomotionClass::Flier => {
            "    \"speed\": { \"surface\": 45, \"air\": 220 },\n    \"gravity\": 0.0,\n    \"air_style\": \"flap_arc\""
        }
        LocomotionClass::Climber => {
            "    \"speed\": { \"surface\": 110, \"climb\": 85 },\n    \"jump\": { \"enabled\": true, \"max_gap\": 160, \"arc_height\": 60 },\n    \"gravity\": 1.0"
        }
        LocomotionClass::Walker => {
            "    \"speed\": { \"surface\": 80 },\n    \"gravity\": 1.0"
        }
        LocomotionClass::Hopper => {
            "    \"speed\": { \"surface\": 90 },\n    \"jump\": { \"enabled\": true, \"max_gap\": 90, \"arc_height\": 50 },\n    \"gravity\": 1.0"
        }
    };
    format!(
        r#"{{
  "$schema": "pet-pack-v1.schema.json",
  "format": 1,
  "id": "com.yourname.{slug}",
  "name": "{name}",
  "author": "Your Name",
  "version": "1.0.0",
  "description": "Describe your pet in a sentence.",
  "license": "CC-BY-4.0",
  "sprite": {{
    "width": {FRAME},
    "height": {FRAME},
    "anchor": {{ "x": 0.5, "y": 1.0 }},
    "default_facing": "right",
    "surface_orientation": "{orientation}"
  }},
  "locomotion": {{
    "class": "{class}",
    "surfaces": {surfaces},
{extra},
    "min_surface_length": 48
  }},
  "animations": {{
{animations}
  }}
}}
"#,
        orientation = if class == LocomotionClass::Flier {
            "upright"
        } else {
            "align"
        },
        class = class.as_str(),
    )
}

fn readme_text(name: &str, class: LocomotionClass) -> String {
    format!(
        "# {name}\n\n\
This pack already runs. Open Pet Lab, pick it, and it appears on your windows.\n\n\
## What to change first\n\n\
1. **The art.** Replace the strips in `sprites/`. Each file is one row of equal-width\n   \
frames; the engine works out the frame width by dividing the image width by `frames`.\n\
2. **`name`, `author`, `id`.** The `id` must be unique — reverse-DNS, like\n   \
`com.yourname.thing`. Two packs with the same id cannot be installed together.\n\
3. **`locomotion.surfaces`.** This does most of the characterisation. A bird takes\n   \
top edges; a gecko takes only `window_vertical`; a squirrel takes everything.\n\n\
## This pack is a {class}\n\n{blurb}\n\n\
## Adding animations\n\n\
Only `idle` and `move` are required. Everything else falls back:\n\
`climb → move → idle`, `land → idle`, `sleep → idle`, and so on. Adding art only ever\n\
improves the pet; leaving it out never breaks the pack.\n\n\
Two of the clips here are for the assistant: `busy` plays while this pet is working on\n\
a task you gave it, and `attention` plays while that task is waiting for your answer.\n\
Both are optional — `busy → preen → idle` and `attention → alert → happy → idle` — and\n\
Pet packs tells you which one a pet is falling back to.\n\n\
## Behaviour\n\n\
Leave `behavior` out and the engine uses a sensible default for this class. When you\n\
want more character, add states with `clip`, `duration`, and weighted `transitions`.\n\
Conditions look like `idle_seconds > 300` and can be joined with `&&` and `||`.\n\
There is no scripting — a pack is data, so it can look wrong but never do harm.\n",
        class = class.as_str(),
        blurb = match class {
            LocomotionClass::Flier =>
                "It ignores the surface graph and flies straight to wherever it wants to land.\nIt never falls.",
            LocomotionClass::Climber =>
                "It walks along edges, climbs window sides, rounds corners and jumps gaps up to\n`max_gap`. If its surface disappears, it falls until something catches it.",
            LocomotionClass::Walker =>
                "It only uses horizontal surfaces that touch, so it needs a connected route.\nIt falls when its surface goes away.",
            LocomotionClass::Hopper =>
                "Like a walker, but it can hop short gaps. Good for frogs and small birds on foot.",
        }
    )
}

/// A recognisable blob so the scaffolded pack looks like something immediately.
fn placeholder(frames: u32, clip: &str) -> Vec<u8> {
    let width = FRAME * frames;
    let mut pixels = vec![0_u8; (width * FRAME * 4) as usize];
    for frame in 0..frames {
        let bob = match clip {
            "idle" => [0_i32, 0, 1, 0][(frame % 4) as usize],
            _ => [0_i32, -2, 0, 2][(frame % 4) as usize],
        };
        for y in 0..FRAME as i32 {
            for x in 0..FRAME as i32 {
                let cx = x - 24;
                let cy = y - 30 + bob;
                let body = (cx * cx) as f64 / 196. + (cy * cy) as f64 / 144.;
                let head = ((x - 32).pow(2) as f64) / 64. + ((y - 16 + bob).pow(2) as f64) / 64.;
                let colour = if body < 1.0 || head < 1.0 {
                    if body < 0.55 || head < 0.5 {
                        [0x7f, 0xa7, 0x8a, 0xff]
                    } else {
                        [0x3f, 0x5f, 0x4e, 0xff]
                    }
                } else {
                    continue;
                };
                let px = (frame * FRAME) as i32 + x;
                let i = ((y as u32 * width + px as u32) * 4) as usize;
                pixels[i..i + 4].copy_from_slice(&colour);
            }
        }
        // One eye, so the placeholder has a face and a direction.
        let eye = ((13 * width + frame * FRAME + 34) * 4) as usize;
        pixels[eye..eye + 4].copy_from_slice(&[0x14, 0x20, 0x1a, 0xff]);
    }
    pixels
}

fn preview_pixels() -> Vec<u8> {
    let mut pixels = vec![0_u8; 256 * 256 * 4];
    for y in 0..256_u32 {
        for x in 0..256_u32 {
            let i = ((y * 256 + x) * 4) as usize;
            pixels[i..i + 4].copy_from_slice(&[0xee, 0xef, 0xe6, 0xff]);
        }
    }
    let sprite = placeholder(1, "idle");
    for y in 0..FRAME {
        for x in 0..FRAME {
            let source = ((y * FRAME + x) * 4) as usize;
            if sprite[source + 3] == 0 {
                continue;
            }
            for dy in 0..3 {
                for dx in 0..3 {
                    let tx = 56 + x * 3 + dx;
                    let ty = 40 + y * 3 + dy;
                    let target = ((ty * 256 + tx) * 4) as usize;
                    pixels[target..target + 4].copy_from_slice(&sprite[source..source + 4]);
                }
            }
        }
    }
    pixels
}

pub(crate) fn write_png(path: &Path, pixels: &[u8], width: u32, height: u32) -> Result<(), String> {
    let file = std::fs::File::create(path)
        .map_err(|e| format!("Could not create {}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?
        .write_image_data(pixels)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))
}
