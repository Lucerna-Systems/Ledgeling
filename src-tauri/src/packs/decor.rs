//! Decor packs: trees, flowers and rocks that stand on a window's top edge in
//! a biome. Data like pet packs, never code, and held to the same rule that
//! every failure names the file, the JSON path, the problem and the fix.
//!
//! A decor pack sits in the packs folder beside the pets and is told apart by
//! its `decor.json`:
//!
//! ```json
//! { "format": 1, "id": "com.perch.decor-meadow", "name": "Meadow",
//!   "author": "Ledgeling", "version": "1.0.0",
//!   "props": {
//!     "oak": { "file": "sprites/oak.png", "width": 48, "height": 64,
//!              "sway": "tall", "perch": { "y": 0.3, "from": 0.15, "to": 0.85 } } } }
//! ```
use super::{
    diagnostics::{Diagnostic, Report},
    limits::{MAX_FPS, MAX_FRAMES, MAX_MANIFEST_BYTES},
    loader::safe_join,
    sprites::{self, Strip},
};
use serde::Serialize;
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub const DECOR_FILE: &str = "decor.json";
pub const FORMAT_VERSION: u64 = 1;
pub const MAX_PROPS: usize = 32;
/// A prop is scenery on a window's edge, not a second pet.
pub const MAX_PROP_SIZE: f64 = 128.;

/// How a prop springs when its window moves. `softness` is how far the
/// window's motion swings it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sway {
    pub stiffness: f64,
    pub damping: f64,
    pub softness: f64,
}

impl Sway {
    pub const PRESETS: [(&'static str, Sway); 3] = [
        (
            "stiff",
            Sway {
                stiffness: 320.,
                damping: 0.6,
                softness: 0.25,
            },
        ),
        (
            "soft",
            Sway {
                stiffness: 140.,
                damping: 0.25,
                softness: 1.,
            },
        ),
        (
            "tall",
            Sway {
                stiffness: 70.,
                damping: 0.2,
                softness: 0.7,
            },
        ),
    ];
    pub fn preset(name: &str) -> Option<Sway> {
        Self::PRESETS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, s)| *s)
    }
}

/// A horizontal ledge across the prop a pet can stand on, as fractions of
/// the prop: `y` from its top, `from`..`to` across its width.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropPerch {
    pub y: f64,
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropDef {
    pub file: String,
    /// Logical pixels.
    pub width: f64,
    pub height: f64,
    /// 0..1 across the prop: the point that stands on the edge.
    pub anchor_x: f64,
    pub frames: u32,
    pub fps: f64,
    pub sway: Sway,
    pub perch: Option<PropPerch>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecorManifest {
    pub format: u64,
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub license: String,
    pub props: BTreeMap<String, PropDef>,
}

#[derive(Clone, Debug)]
pub struct DecorPack {
    pub manifest: DecorManifest,
    pub directory: PathBuf,
    pub strips: BTreeMap<String, Strip>,
    pub builtin: bool,
}

/// What the overlay needs to draw one prop.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropAtlas {
    pub image: String,
    pub frames: u32,
    pub fps: f64,
    pub width: f64,
    pub height: f64,
}

impl DecorPack {
    pub fn atlas(&self) -> BTreeMap<String, PropAtlas> {
        self.manifest
            .props
            .iter()
            .filter_map(|(name, prop)| {
                let strip = self.strips.get(name)?;
                Some((
                    name.clone(),
                    PropAtlas {
                        image: strip.data_uri.clone(),
                        frames: prop.frames,
                        fps: prop.fps,
                        width: prop.width,
                        height: prop.height,
                    },
                ))
            })
            .collect()
    }
}

pub fn is_decor_directory(directory: &Path) -> bool {
    directory.join(DECOR_FILE).is_file()
}

/// Every loadable decor pack in the packs folder, plus the reports of any
/// that failed.
pub fn scan(packs_dir: &Path) -> (Vec<DecorPack>, Vec<Report>) {
    let mut packs = Vec::new();
    let mut broken = Vec::new();
    let Ok(entries) = std::fs::read_dir(packs_dir) else {
        return (packs, broken);
    };
    let mut directories: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_decor_directory(p))
        .collect();
    directories.sort();
    for directory in directories {
        let builtin = super::builtin::is_builtin_directory(&directory);
        match load(&directory, builtin) {
            Ok(pack) => packs.push(pack),
            Err(report) => broken.push(report),
        }
    }
    (packs, broken)
}

pub fn load(directory: &Path, builtin: bool) -> Result<DecorPack, Report> {
    let mut report = Report::default();
    let path = directory.join(DECOR_FILE);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_MANIFEST_BYTES) {
        report.push(error(
            "",
            "The decor file is unreasonably large.",
            format!("Keep {DECOR_FILE} under {} KiB.", MAX_MANIFEST_BYTES >> 10),
        ));
        return Err(report);
    }
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => {
            report.push(error(
                "",
                format!("The decor file could not be read: {e}."),
                format!("A decor pack needs a `{DECOR_FILE}` beside its `sprites` folder."),
            ));
            return Err(report);
        }
    };
    let Some(manifest) = parse(&text, &mut report) else {
        return Err(report);
    };
    let mut strips = BTreeMap::new();
    for (name, prop) in &manifest.props {
        let Some(file) = safe_join(directory, &prop.file) else {
            report.push(error(
                format!("props.{name}.file"),
                format!("`{}` points outside the pack folder.", prop.file),
                "Sprite paths are relative to the pack, e.g. `sprites/oak.png`.",
            ));
            continue;
        };
        if let Some(strip) = sprites::load(&file, &prop.file, prop.frames, &mut report) {
            strips.insert(name.clone(), strip);
        }
    }
    if report.has_errors() {
        return Err(report);
    }
    Ok(DecorPack {
        manifest,
        directory: directory.to_path_buf(),
        strips,
        builtin,
    })
}

pub fn parse(text: &str, report: &mut Report) -> Option<DecorManifest> {
    let value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(e) => {
            report.push(error(
                "",
                format!(
                    "The file is not valid JSON at line {}, column {}.",
                    e.line(),
                    e.column()
                ),
                "Check for a missing comma, a trailing comma, or an unclosed bracket.",
            ));
            return None;
        }
    };
    let Some(root) = value.as_object() else {
        report.push(error(
            "",
            "Expected an object.",
            "Wrap the whole file in `{ ... }`.",
        ));
        return None;
    };
    let format = number(root, "", "format", false, report).unwrap_or(FORMAT_VERSION as f64);
    if format != FORMAT_VERSION as f64 {
        report.push(error(
            "format",
            format!("This pack declares format {format}."),
            format!("This build understands format {FORMAT_VERSION}."),
        ));
    }
    let id = text_field(root, "", "id", true, report)?;
    let name = text_field(root, "", "name", true, report)?;
    let author = text_field(root, "", "author", true, report).unwrap_or_default();
    let version = text_field(root, "", "version", true, report).unwrap_or_default();
    let license = text_field(root, "", "license", false, report).unwrap_or_default();
    let mut props = BTreeMap::new();
    match root.get("props").and_then(Value::as_object) {
        None => report.push(error(
            "props",
            "There are no props.",
            "Add `\"props\": { \"oak\": { \"file\": \"sprites/oak.png\", \"width\": 48, \"height\": 64 } }`.",
        )),
        Some(map) if map.len() > MAX_PROPS => report.push(error(
            "props",
            format!("The pack has {} props.", map.len()),
            format!("Keep it to {MAX_PROPS} props or fewer; split the rest into another pack."),
        )),
        Some(map) => {
            for (key, value) in map {
                if let Some(prop) = read_prop(key, value, report) {
                    props.insert(key.clone(), prop);
                }
            }
        }
    }
    note_unknown(
        root,
        "",
        &[
            "$schema", "format", "id", "name", "author", "version", "license", "props",
        ],
        report,
    );
    Some(DecorManifest {
        format: FORMAT_VERSION,
        id,
        name,
        author,
        version,
        license,
        props,
    })
}

fn read_prop(key: &str, value: &Value, report: &mut Report) -> Option<PropDef> {
    let path = format!("props.{key}");
    let Some(map) = value.as_object() else {
        report.push(error(
            path,
            "Expected an object.",
            "Describe the prop as `{ \"file\": ..., \"width\": ..., \"height\": ... }`.",
        ));
        return None;
    };
    let file = text_field(map, &path, "file", true, report)?;
    let mut size = |field: &str| {
        let v = number(map, &path, field, true, report)?;
        if !(1. ..=MAX_PROP_SIZE).contains(&v) {
            report.push(error(
                format!("{path}.{field}"),
                format!("Found {v}."),
                format!("Props are 1 to {MAX_PROP_SIZE} logical pixels on each side."),
            ));
            return None;
        }
        Some(v)
    };
    let (width, height) = (size("width"), size("height"));
    let anchor_x = fraction(map, &path, "anchorX", report).unwrap_or(0.5);
    let frames = number(map, &path, "frames", false, report).unwrap_or(1.);
    if frames < 1. || frames > MAX_FRAMES as f64 || frames.fract() != 0. {
        report.push(error(
            format!("{path}.frames"),
            format!("Found {frames}."),
            format!("A whole number from 1 to {MAX_FRAMES}."),
        ));
    }
    let fps = number(map, &path, "fps", false, report)
        .unwrap_or(0.)
        .clamp(0., MAX_FPS);
    let sway = match map.get("sway") {
        None => Sway::preset("soft").unwrap(),
        Some(Value::String(name)) => Sway::preset(name).unwrap_or_else(|| {
            report.push(error(
                format!("{path}.sway"),
                format!("Found `{name}`."),
                "Expected `stiff`, `soft` or `tall`, or `{ \"stiffness\": 140, \"damping\": 0.25 }`.",
            ));
            Sway::preset("soft").unwrap()
        }),
        Some(Value::Object(custom)) => {
            let base = Sway::preset("soft").unwrap();
            let sway_path = format!("{path}.sway");
            Sway {
                stiffness: number(custom, &sway_path, "stiffness", false, report)
                    .unwrap_or(base.stiffness)
                    .clamp(10., 600.),
                damping: number(custom, &sway_path, "damping", false, report)
                    .unwrap_or(base.damping)
                    .clamp(0.05, 1.5),
                softness: number(custom, &sway_path, "softness", false, report)
                    .unwrap_or(base.softness)
                    .clamp(0., 2.),
            }
        }
        Some(_) => {
            report.push(error(
                format!("{path}.sway"),
                "Expected a preset name or an object.",
                "Use `\"sway\": \"soft\"` or `{ \"stiffness\": 140, \"damping\": 0.25 }`.",
            ));
            Sway::preset("soft").unwrap()
        }
    };
    let perch = match map.get("perch") {
        None => None,
        Some(Value::Object(ledge)) => {
            let perch_path = format!("{path}.perch");
            let y = fraction(ledge, &perch_path, "y", report);
            let from = fraction(ledge, &perch_path, "from", report).unwrap_or(0.);
            let to = fraction(ledge, &perch_path, "to", report).unwrap_or(1.);
            if from >= to {
                report.push(error(
                    format!("{perch_path}.to"),
                    format!("The ledge runs from {from} to {to}."),
                    "`to` must be greater than `from`.",
                ));
            }
            note_unknown(ledge, &perch_path, &["y", "from", "to"], report);
            match y {
                Some(y) => Some(PropPerch { y, from, to }),
                None => {
                    report.push(error(
                        format!("{perch_path}.y"),
                        "The ledge has no height.",
                        "Give `y`, from 0 at the top of the prop to 1 at its base.",
                    ));
                    None
                }
            }
        }
        Some(_) => {
            report.push(error(
                format!("{path}.perch"),
                "Expected an object.",
                "Use `\"perch\": { \"y\": 0.3, \"from\": 0.2, \"to\": 0.8 }`.",
            ));
            None
        }
    };
    note_unknown(
        map,
        &path,
        &[
            "file", "width", "height", "anchorX", "frames", "fps", "sway", "perch",
        ],
        report,
    );
    Some(PropDef {
        file,
        width: width?,
        height: height?,
        anchor_x,
        frames: frames.clamp(1., MAX_FRAMES as f64) as u32,
        fps,
        sway,
        perch,
    })
}

fn error(
    path: impl Into<String>,
    problem: impl Into<String>,
    expected: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(DECOR_FILE, path, problem, expected)
}

fn full(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

fn text_field(
    map: &Map<String, Value>,
    path: &str,
    key: &str,
    required: bool,
    report: &mut Report,
) -> Option<String> {
    match map.get(key) {
        Some(Value::String(text)) => Some(text.clone()),
        None if !required => None,
        None => {
            report.push(error(
                full(path, key),
                format!("The required field `{key}` is missing."),
                "Expected a piece of text in quotes.",
            ));
            None
        }
        Some(_) => {
            report.push(error(
                full(path, key),
                "Expected text.",
                "Put the value in double quotes.",
            ));
            None
        }
    }
}

fn number(
    map: &Map<String, Value>,
    path: &str,
    key: &str,
    required: bool,
    report: &mut Report,
) -> Option<f64> {
    match map.get(key) {
        Some(Value::Number(n)) => n.as_f64(),
        None if !required => None,
        None => {
            report.push(error(
                full(path, key),
                format!("The required field `{key}` is missing."),
                "Expected a number.",
            ));
            None
        }
        Some(_) => {
            report.push(error(
                full(path, key),
                "Expected a number.",
                "Numbers are written without quotes, e.g. 48.",
            ));
            None
        }
    }
}

fn fraction(map: &Map<String, Value>, path: &str, key: &str, report: &mut Report) -> Option<f64> {
    let v = number(map, path, key, false, report)?;
    if !(0. ..=1.).contains(&v) {
        report.push(error(
            full(path, key),
            format!("Found {v}."),
            "A fraction from 0 to 1.",
        ));
        return None;
    }
    Some(v)
}

fn note_unknown(map: &Map<String, Value>, path: &str, known: &[&str], report: &mut Report) {
    for key in map.keys().filter(|k| !known.contains(&k.as_str())) {
        report.push(Diagnostic::info(
            DECOR_FILE,
            full(path, key),
            format!("Unknown field `{key}` was ignored."),
            "Check the spelling, or ignore this if the pack targets a newer Ledgeling.",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEADOW: &str = r#"{
      "format": 1, "id": "com.example.decor", "name": "Test", "author": "A", "version": "1.0.0",
      "props": {
        "oak": { "file": "sprites/oak.png", "width": 48, "height": 64, "sway": "tall",
                 "perch": { "y": 0.3, "from": 0.2, "to": 0.8 } },
        "tuft": { "file": "sprites/tuft.png", "width": 16, "height": 10 }
      }
    }"#;

    fn parsed(text: &str) -> (Option<DecorManifest>, Report) {
        let mut report = Report::default();
        (parse(text, &mut report), report)
    }

    fn paths(report: &Report) -> Vec<String> {
        report.errors().map(|d| d.path.clone()).collect()
    }

    #[test]
    fn a_minimal_decor_pack_parses_with_sensible_defaults() {
        let (manifest, report) = parsed(MEADOW);
        let manifest = manifest.unwrap();
        assert!(!report.has_errors(), "{}", report.text());
        let oak = &manifest.props["oak"];
        assert_eq!(oak.sway, Sway::preset("tall").unwrap());
        assert_eq!(oak.perch.unwrap().y, 0.3);
        let tuft = &manifest.props["tuft"];
        assert_eq!((tuft.anchor_x, tuft.frames), (0.5, 1));
        assert!(tuft.perch.is_none());
    }

    #[test]
    fn every_bad_prop_value_names_its_path_and_file() {
        let bad = MEADOW
            .replace("\"tall\"", "\"wobbly\"")
            .replace("\"from\": 0.2", "\"from\": 1.5")
            .replace("\"width\": 16", "\"width\": 400");
        let (_, report) = parsed(&bad);
        let found = paths(&report);
        for path in ["props.oak.sway", "props.oak.perch.from", "props.tuft.width"] {
            assert!(found.iter().any(|p| p == path), "{path}: {}", report.text());
        }
        assert!(report.errors().all(|d| d.file == DECOR_FILE));
        let sway = report
            .errors()
            .find(|d| d.path == "props.oak.sway")
            .unwrap();
        assert!(sway.expected.contains("stiff"), "{}", sway.expected);
    }

    #[test]
    fn a_backwards_ledge_and_too_many_props_are_refused() {
        let (_, report) = parsed(&MEADOW.replace("\"to\": 0.8", "\"to\": 0.1"));
        assert!(paths(&report).contains(&"props.oak.perch.to".to_string()));
        let many: Vec<String> = (0..=MAX_PROPS)
            .map(|i| {
                format!("\"p{i}\": {{ \"file\": \"sprites/p.png\", \"width\": 8, \"height\": 8 }}")
            })
            .collect();
        let text = format!(
            "{{ \"id\": \"a.b\", \"name\": \"n\", \"author\": \"a\", \"version\": \"1.0.0\", \"props\": {{ {} }} }}",
            many.join(",")
        );
        let (_, report) = parsed(&text);
        assert!(
            paths(&report).contains(&"props".to_string()),
            "{}",
            report.text()
        );
    }

    #[test]
    fn a_missing_sprite_is_named_and_the_pack_does_not_load() {
        let directory = std::env::temp_dir().join(format!("decor-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(DECOR_FILE), MEADOW).unwrap();
        let report = load(&directory, false).expect_err("no sprites on disk");
        assert!(
            report.errors().any(|d| d.file == "sprites/oak.png"),
            "{}",
            report.text()
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn the_bundled_meadow_loads_cleanly() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../packs/decor-meadow");
        let pack = load(&directory, true).unwrap_or_else(|r| panic!("{}", r.text()));
        assert!(pack.manifest.props.len() >= 5);
        assert!(pack.manifest.props.values().any(|p| p.perch.is_some()));
        assert_eq!(pack.atlas().len(), pack.manifest.props.len());
    }
}
