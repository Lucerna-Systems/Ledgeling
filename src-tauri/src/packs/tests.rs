//! Pack loading, validation and the behavior format. Every malformed case must
//! produce a message that names the file, the path, the problem and the fix.
use super::{
    diagnostics::{Diagnostic, Report, Tier},
    loader,
    manifest::{self, LocomotionClass, MANIFEST_FILE},
    scaffold,
};
use crate::world::SurfaceKind;
use std::path::{Path, PathBuf};

const MINIMAL: &str = r#"{
  "format": 1,
  "id": "com.yourname.simplebird",
  "name": "Simple Bird",
  "author": "Your Name",
  "version": "1.0.0",
  "sprite": { "width": 48, "height": 48, "anchor": { "x": 0.5, "y": 1.0 } },
  "locomotion": { "class": "flier" },
  "animations": {
    "idle": { "file": "sprites/idle.png", "frames": 4, "fps": 6 },
    "move": { "file": "sprites/move.png", "frames": 6, "fps": 14 }
  }
}"#;

fn parse(text: &str) -> (Option<manifest::Manifest>, Report) {
    let mut report = Report::default();
    let manifest = manifest::parse(text, &mut report);
    (manifest, report)
}

fn errors(report: &Report) -> Vec<String> {
    report.errors().map(|d| d.to_string()).collect()
}

fn repo_pack(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../packs")
        .join(name)
}

#[test]
fn the_minimal_pack_from_the_spec_is_a_complete_working_pet() {
    let (manifest, report) = parse(MINIMAL);
    let manifest = manifest.expect("a minimal pack loads");
    assert!(!report.has_errors(), "{}", report.text());
    assert_eq!(manifest.locomotion.class, LocomotionClass::Flier);
    // Everything unstated comes from the class defaults.
    assert!(manifest.allows(SurfaceKind::WindowTop));
    assert!(!manifest.allows(SurfaceKind::WindowLeft));
    assert_eq!(manifest.locomotion.gravity, 0.);
    assert!(
        !manifest.behavior.states.is_empty(),
        "a default profile is supplied"
    );
    assert!(!manifest.reactions.is_empty() || manifest.reactions.is_empty());
}

#[test]
fn a_pack_can_name_its_own_pet_type_and_actions() {
    let text = MINIMAL
        .replace(
            r#""class": "flier""#,
            r#""class": "walker", "type": "slitherer""#,
        )
        .replace(
            r#""animations": {"#,
            r#""actions": { "scurry": { "base": "patrol_current_surface", "speed": 2.5 } },
  "behavior": { "initial": "run", "states": {
    "run": { "clip": "move", "action": "scurry", "transitions": [{ "to": "run" }] } } },
  "animations": {"#,
        );
    let (manifest, report) = parse(&text);
    let manifest = manifest.expect("loads");
    assert!(!report.has_errors(), "{}", report.text());
    assert_eq!(manifest.locomotion.kind, "slitherer");
    let state = &manifest.behavior.states["run"];
    assert_eq!(state.action, Some(manifest::Action::PatrolCurrentSurface));
    assert_eq!(state.speed_scale, 2.5);
    // Without a declared type the class name stands in.
    let (plain, _) = parse(MINIMAL);
    assert_eq!(plain.unwrap().locomotion.kind, "flier");
}

#[test]
fn a_bad_custom_action_names_the_path_and_the_fix() {
    let text = MINIMAL.replace(
        r#""animations": {"#,
        r#""actions": { "flee_from_cursor": { "base": "patrol_current_surface" },
                  "zoom": { "base": "teleport" } },
  "behavior": { "initial": "a", "states": { "a": { "action": "nope" } } },
  "animations": {"#,
    );
    let (_, report) = parse(&text);
    let all = errors(&report).join("\n");
    assert!(
        all.contains("actions.flee_from_cursor") && all.contains("built-in"),
        "{all}"
    );
    assert!(
        all.contains("actions.zoom.base") && all.contains("Expected one of"),
        "{all}"
    );
    assert!(
        all.contains("behavior.states.a.action") && all.contains("`actions` block"),
        "{all}"
    );
}

#[test]
fn missing_animations_fall_back_down_the_chain() {
    let (manifest, _) = parse(MINIMAL);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.resolve_clip("climb"), "move");
    assert_eq!(manifest.resolve_clip("land"), "idle");
    assert_eq!(manifest.resolve_clip("sleep"), "idle");
    assert_eq!(manifest.resolve_clip("move"), "move");
    assert_eq!(manifest.resolve_clip("something-invented"), "idle");
}

#[test]
fn an_empty_assistant_clip_slot_is_reported_with_what_plays_instead() {
    let (manifest, report) = parse(MINIMAL);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.resolve_clip("busy"), "idle");
    assert_eq!(manifest.resolve_clip("attention"), "idle");
    let notes: Vec<&Diagnostic> = report
        .diagnostics
        .iter()
        .filter(|d| d.tier == Tier::Info)
        .collect();
    for slot in ["busy", "attention"] {
        let note = notes
            .iter()
            .find(|d| d.path == format!("animations.{slot}"))
            .unwrap_or_else(|| panic!("no note about the empty `{slot}` slot"));
        assert!(note.problem.contains("plays `idle`"), "{}", note.problem);
        assert!(note.expected.contains(&format!("animations.{slot}")));
    }
    // An empty optional slot is a note, never something that blocks the pack.
    assert!(!report.has_errors());
}

#[test]
fn a_pack_that_supplies_its_own_assistant_clips_is_not_nagged() {
    let text = MINIMAL.replace(
        "\"move\": { \"file\": \"sprites/move.png\", \"frames\": 6, \"fps\": 14 }",
        "\"move\": { \"file\": \"sprites/move.png\", \"frames\": 6, \"fps\": 14 },\n    \"busy\": { \"file\": \"sprites/busy.png\", \"frames\": 4, \"fps\": 8 },\n    \"attention\": { \"file\": \"sprites/attention.png\", \"frames\": 4, \"fps\": 6 }",
    );
    let (manifest, report) = parse(&text);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.resolve_clip("busy"), "busy");
    assert_eq!(manifest.resolve_clip("attention"), "attention");
    assert!(!report
        .diagnostics
        .iter()
        .any(|d| d.path.starts_with("animations.busy")));
}

#[test]
fn unknown_fields_are_ignored_with_a_note_rather_than_failing() {
    let text = MINIMAL.replace(
        "\"format\": 1,",
        "\"format\": 1,\n  \"holographic\": true,\n  \"smell\": \"pine\",",
    );
    let (manifest, report) = parse(&text);
    assert!(manifest.is_some(), "a newer pack still loads");
    let notes: Vec<String> = report
        .diagnostics
        .iter()
        .filter(|d| d.tier == Tier::Info)
        .map(|d| d.problem.clone())
        .collect();
    assert!(notes.iter().any(|n| n.contains("holographic")));
    assert!(notes.iter().any(|n| n.contains("smell")));
}

#[test]
fn broken_json_is_explained_in_english_with_a_line_number() {
    let (manifest, report) = parse("{ \"format\": 1, \"id\": }");
    assert!(manifest.is_none());
    let text = report.text();
    assert!(text.contains(MANIFEST_FILE));
    assert!(text.contains("not valid JSON"));
    assert!(text.contains("line 1"));
    assert!(
        !text.contains("expected value at line") || text.contains("Check for"),
        "the raw serde message is never the whole message"
    );
}

#[test]
fn a_quoted_frame_count_names_the_path_the_problem_and_the_fix() {
    let text = MINIMAL.replace("\"frames\": 4", "\"frames\": \"6 \"");
    let (_, report) = parse(&text);
    let message = errors(&report).join("\n");
    assert!(message.contains("animations.idle.frames"), "{message}");
    assert!(
        message.contains("Expected a positive integer, found \"6 \" (string)"),
        "{message}"
    );
    assert!(message.contains("not quoted strings"), "{message}");
}

#[test]
fn every_required_field_missing_produces_its_own_error() {
    for (field, needle) in [
        ("\"id\": \"com.yourname.simplebird\",", "id"),
        ("\"name\": \"Simple Bird\",", "name"),
        ("\"author\": \"Your Name\",", "author"),
        ("\"version\": \"1.0.0\",", "version"),
    ] {
        let (_, report) = parse(&MINIMAL.replace(field, ""));
        let message = errors(&report).join("\n");
        assert!(
            message.contains(&format!("The required field `{needle}` is missing")),
            "removing {needle} should say so: {message}"
        );
    }
}

#[test]
fn a_missing_locomotion_block_says_what_to_add() {
    let (_, report) = parse(&MINIMAL.replace("\"locomotion\": { \"class\": \"flier\" },", ""));
    let message = errors(&report).join("\n");
    assert!(message.contains("locomotion block is missing"), "{message}");
    assert!(
        message.contains("flier, climber, walker, hopper"),
        "{message}"
    );
}

#[test]
fn an_unknown_class_or_surface_lists_the_valid_ones() {
    let (_, report) = parse(&MINIMAL.replace("\"flier\"", "\"swimmer\""));
    let message = errors(&report).join("\n");
    assert!(message.contains("locomotion.class"), "{message}");
    assert!(message.contains("walker"), "{message}");

    let text = MINIMAL.replace(
        "{ \"class\": \"flier\" }",
        "{ \"class\": \"flier\", \"surfaces\": [\"window_middle\"] }",
    );
    let (_, report) = parse(&text);
    let message = errors(&report).join("\n");
    assert!(
        message.contains("Unknown surface `window_middle`"),
        "{message}"
    );
    assert!(message.contains("window_any"), "{message}");
}

#[test]
fn a_pack_with_only_idle_loads_and_uses_it_for_everything() {
    let text = MINIMAL.replace(
        ",\n    \"move\": { \"file\": \"sprites/move.png\", \"frames\": 6, \"fps\": 14 }",
        "",
    );
    let (manifest, report) = parse(&text);
    assert!(!report.has_errors(), "{}", report.text());
    let manifest = manifest.unwrap();
    assert_eq!(manifest.resolve_clip("move"), "idle");
    assert_eq!(manifest.resolve_clip("climb"), "idle");
}

#[test]
fn a_pack_with_only_move_plays_it_in_place_of_idle() {
    let text = MINIMAL.replace(
        "\"idle\": { \"file\": \"sprites/idle.png\", \"frames\": 4, \"fps\": 6 },\n    ",
        "",
    );
    let (manifest, report) = parse(&text);
    assert!(!report.has_errors(), "{}", report.text());
    let manifest = manifest.unwrap();
    assert_eq!(manifest.resolve_clip("idle"), "move");
    assert_eq!(manifest.resolve_clip("sleep"), "move");
}

#[test]
fn a_pack_with_some_other_clip_still_loads() {
    let text = MINIMAL
        .replace(
            "\"move\": { \"file\": \"sprites/move.png\", \"frames\": 6, \"fps\": 14 }",
            "\"flap\": { \"file\": \"sprites/move.png\", \"frames\": 6, \"fps\": 14 }",
        )
        .replace(
            "\"idle\": { \"file\": \"sprites/idle.png\", \"frames\": 4, \"fps\": 6 },",
            "",
        );
    let (manifest, report) = parse(&text);
    assert!(!report.has_errors(), "{}", report.text());
    assert_eq!(manifest.unwrap().resolve_clip("idle"), "flap");
}

#[test]
fn a_pack_with_no_animations_is_an_error() {
    let start = MINIMAL.find("\"animations\"").unwrap();
    let text = format!("{}\"animations\": {{}}\n}}", &MINIMAL[..start]);
    let (_, report) = parse(&text);
    let message = errors(&report).join("\n");
    assert!(message.contains("no animations"), "{message}");
}

#[test]
fn an_unparseable_condition_is_a_validation_error_not_a_silent_no_op() {
    let text = MINIMAL.replace(
        "\"animations\"",
        r#""behavior": {
    "initial": "perch",
    "states": {
      "perch": { "clip": "idle", "duration": [1, 2],
        "transitions": [{ "to": "perch", "when": "idle_secondz > 5" }] }
    }
  },
  "animations""#,
    );
    let (_, report) = parse(&text);
    let message = errors(&report).join("\n");
    assert!(
        message.contains("Unknown signal `idle_secondz`"),
        "{message}"
    );
    assert!(message.contains("idle_seconds"), "{message}");
}

#[test]
fn transitions_and_reactions_must_name_states_that_exist() {
    let text = MINIMAL.replace(
        "\"animations\"",
        r#""behavior": {
    "initial": "perch",
    "states": { "perch": { "clip": "idle", "transitions": [{ "to": "moonwalk" }] } }
  },
  "reactions": [{ "on": "surface_lost", "goto": "vanish" }],
  "animations""#,
    );
    let (_, report) = parse(&text);
    let message = errors(&report).join("\n");
    assert!(
        message.contains("`moonwalk` is not a declared state"),
        "{message}"
    );
    assert!(
        message.contains("`vanish` is not a declared state"),
        "{message}"
    );
}

#[test]
fn an_unknown_event_or_action_lists_the_catalogue() {
    let text = MINIMAL.replace(
        "\"animations\"",
        r#""behavior": {
    "initial": "perch",
    "states": { "perch": { "action": "teleport", "transitions": [] } }
  },
  "reactions": [{ "on": "user_sneezed", "goto": "perch" }],
  "animations""#,
    );
    let (_, report) = parse(&text);
    let message = errors(&report).join("\n");
    assert!(message.contains("Unknown action `teleport`"), "{message}");
    assert!(message.contains("move_to_random_surface"), "{message}");
    assert!(
        message.contains("Unknown event `user_sneezed`"),
        "{message}"
    );
    assert!(message.contains("break_taken"), "{message}");
}

#[test]
fn no_diagnostic_is_ever_only_a_raw_error_or_a_stack_trace() {
    let broken = [
        MINIMAL.replace("48,", "\"48\","),
        MINIMAL.replace("\"frames\": 4", "\"frames\": -2"),
        MINIMAL.replace("\"fps\": 6", "\"fps\": 0"),
        MINIMAL.replace("\"sprite\":", "\"sprite\": 12, \"unused\":"),
        MINIMAL.replace("\"animations\": {", "\"animations\": ["),
        "{}".to_string(),
        "not json at all".to_string(),
    ];
    for text in broken {
        let (_, report) = parse(&text);
        assert!(report.has_errors(), "expected an error for: {text}");
        for diagnostic in report.errors() {
            assert!(!diagnostic.file.is_empty(), "an error must name a file");
            assert!(
                !diagnostic.expected.is_empty(),
                "an error must say what was expected: {diagnostic}"
            );
            assert!(
                !diagnostic.problem.contains("panicked")
                    && !diagnostic.problem.contains("backtrace"),
                "no stack traces: {diagnostic}"
            );
            assert!(
                diagnostic.problem.ends_with('.') || diagnostic.problem.ends_with(')'),
                "errors read as sentences: {diagnostic}"
            );
        }
    }
}

#[test]
fn omitting_behavior_yields_a_sensible_default_per_class() {
    for class in LocomotionClass::ALL {
        let text = MINIMAL.replace("\"flier\"", &format!("\"{}\"", class.as_str()));
        let (manifest, _) = parse(&text);
        let manifest = manifest.unwrap();
        assert!(
            manifest
                .behavior
                .states
                .contains_key(&manifest.behavior.initial),
            "{} has a usable initial state",
            class.as_str()
        );
        assert!(manifest.behavior.states.len() >= 3, "{}", class.as_str());
    }
}

// ------------------------------------------------------------- bundled packs

#[test]
fn the_bundled_packs_load_without_errors() {
    for name in ["budgie-classic", "squirrel-red", "gecko-glass", "cat-tabby"] {
        let pack = loader::load(&repo_pack(name), true)
            .unwrap_or_else(|report| panic!("{name} should load:\n{}", report.text()));
        assert!(!pack.strips.is_empty());
        assert!(pack.preview.is_some(), "{name} needs a preview");
        assert!(
            pack.strips.contains_key("idle") && pack.strips.contains_key("move"),
            "{name} has the required clips"
        );
    }
}

#[test]
fn the_two_shipped_species_differ_only_by_data() {
    let budgie = loader::load(&repo_pack("budgie-classic"), true).unwrap();
    let squirrel = loader::load(&repo_pack("squirrel-red"), true).unwrap();
    assert_eq!(budgie.manifest.locomotion.class, LocomotionClass::Flier);
    assert_eq!(squirrel.manifest.locomotion.class, LocomotionClass::Climber);
    assert!(budgie.manifest.allows(SurfaceKind::WindowTop));
    assert!(!budgie.manifest.allows(SurfaceKind::WindowLeft));
    assert!(squirrel.manifest.allows(SurfaceKind::WindowLeft));
    assert!(squirrel.manifest.locomotion.jump.enabled);
    assert!(!budgie.manifest.locomotion.jump.enabled);
    assert_eq!(budgie.manifest.locomotion.gravity, 0.);
    assert_eq!(squirrel.manifest.locomotion.gravity, 1.);
}

#[test]
fn a_third_species_needs_no_code_only_two_strips_and_a_manifest() {
    // The gecko is the format's acceptance test: vertical edges only, no jump,
    // rotated flat to the glass, and nothing the engine had to learn.
    let gecko = loader::load(&repo_pack("gecko-glass"), true).unwrap();
    // Two required strips; dizzy and petted are optional extras.
    assert!(gecko.manifest.animations.contains_key("idle"));
    assert!(gecko.manifest.animations.contains_key("move"));
    assert_eq!(gecko.manifest.animations.len(), 4);
    assert_eq!(
        gecko.manifest.locomotion.surfaces,
        vec![SurfaceKind::WindowLeft, SurfaceKind::WindowRight]
    );
    assert!(!gecko.manifest.locomotion.jump.enabled);
    assert_eq!(
        gecko.manifest.sprite.surface_orientation,
        crate::packs::SurfaceOrientation::Align
    );
    assert!(!gecko.manifest.behavior.states.is_empty());
    let lines = std::fs::read_to_string(repo_pack("gecko-glass").join(MANIFEST_FILE))
        .unwrap()
        .lines()
        .count();
    assert!(lines < 40, "the whole pack is {lines} lines of JSON");
}

// ---------------------------------------------------------------- scaffolding

#[test]
fn a_scaffolded_pack_runs_immediately_without_edits() {
    let root = std::env::temp_dir().join(format!("perch-scaffold-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    for class in LocomotionClass::ALL {
        let directory = scaffold::create(&root, class, &format!("Test {}", class.as_str()))
            .expect("scaffolding succeeds");
        let pack = loader::load(&directory, false).unwrap_or_else(|report| {
            panic!(
                "{} scaffold should load:\n{}",
                class.as_str(),
                report.text()
            )
        });
        assert_eq!(pack.manifest.locomotion.class, class);
        assert!(directory.join("README.md").exists());
        assert!(directory.join("pet-pack-v1.schema.json").exists());
        assert!(pack.preview.is_some());
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_pack_folder_may_not_reach_outside_itself() {
    assert!(loader::safe_join(Path::new("/packs/a"), "../../etc/passwd").is_none());
    assert!(loader::safe_join(Path::new("/packs/a"), "/etc/passwd").is_none());
    assert_eq!(
        loader::safe_join(Path::new("/packs/a"), "sprites/idle.png"),
        Some(PathBuf::from("/packs/a/sprites/idle.png"))
    );
}

#[test]
fn duplicate_pack_ids_are_rejected_with_the_name_of_the_installed_one() {
    let root = std::env::temp_dir().join(format!("perch-install-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let first = loader::install(&repo_pack("gecko-glass"), &root).expect("first install");
    assert_eq!(first.id, "com.perch.gecko-glass");
    let error = loader::install(&repo_pack("gecko-glass"), &root)
        .unwrap_err()
        .to_string();
    assert!(error.contains("already installed"), "{error}");
    assert!(error.contains("Glass the Gecko"), "{error}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn exported_packs_round_trip_through_the_petpack_archive() {
    let root = std::env::temp_dir().join(format!("perch-export-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let archive = root.join("gecko.petpack");
    loader::export(&repo_pack("gecko-glass"), &archive).expect("export");
    let installed = loader::install(&archive, &root).expect("install from archive");
    assert_eq!(installed.id, "com.perch.gecko-glass");
    assert_eq!(installed.class, "climber");
    let _ = std::fs::remove_dir_all(&root);
}

mod studio_saves {
    use super::super::studio::{save, StudioImage};
    use std::path::{Path, PathBuf};

    fn image(name: &str, frames: u32) -> StudioImage {
        let (width, height) = (32 * frames, 32);
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        // One opaque pixel per frame so the strip is not blank.
        for frame in 0..frames {
            let at = ((16 * width + frame * 32 + 16) * 4) as usize;
            pixels[at..at + 4].copy_from_slice(&[200, 80, 40, 255]);
        }
        StudioImage {
            name: name.into(),
            width,
            height,
            pixels,
        }
    }
    fn manifest(id: &str, name: &str) -> String {
        format!(
            r#"{{ "format": 1, "id": "{id}", "name": "{name}", "author": "Me", "version": "1.0.0",
  "sprite": {{ "width": 32, "height": 32, "anchor": {{ "x": 0.5, "y": 1.0 }} }},
  "locomotion": {{ "class": "walker" }},
  "animations": {{
    "idle": {{ "file": "sprites/idle.png", "frames": 2, "fps": 6 }},
    "move": {{ "file": "sprites/move.png", "frames": 1, "fps": 8 }}
  }},
  "behavior": {{ "initial": "rest", "states": {{
    "rest": {{ "clip": "idle", "duration": [2, 4], "transitions": [{{ "to": "wander", "weight": 1, "when": "idle_seconds < 60 && energy > 0.2" }}] }},
    "wander": {{ "clip": "move", "action": "move_to_random_surface", "transitions": [{{ "to": "rest", "weight": 1 }}] }}
  }} }},
  "reactions": [{{ "on": "clicked", "goto": "rest", "cooldown": 3 }}]
}}"#
        )
    }
    fn root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("perch-studio-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn a_studio_design_becomes_a_loadable_pack() {
        let root = root("ok");
        let summary = save(
            &root,
            &manifest("com.studio.pebble", "Pebble"),
            &[image("idle", 2), image("move", 1)],
            &image("preview", 1),
        )
        .unwrap();
        assert_eq!(summary.name, "Pebble");
        assert!(summary.loadable);
        assert!(root.join("com.studio.pebble/sprites/idle.png").exists());
        assert!(!root.join(".studio-staging").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_saved_design_opens_again_with_its_pixels() {
        use super::super::studio::open;
        let root = root("open");
        // A 48 px pack written the way Studio writes it.
        let (width, height) = (48u32 * 2, 48u32);
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        pixels[((20 * width + 60) * 4) as usize..][..4].copy_from_slice(&[9, 8, 7, 255]);
        let strip = StudioImage {
            name: "idle".into(),
            width,
            height,
            pixels: pixels.clone(),
        };
        let move_strip = StudioImage {
            name: "move".into(),
            width: 48,
            height,
            pixels: vec![255; 48 * 48 * 4],
        };
        let text = manifest("com.studio.pebble", "Pebble").replace(
            "\"width\": 32, \"height\": 32",
            "\"width\": 48, \"height\": 48",
        );
        save(&root, &text, &[strip, move_strip], &image("preview", 1)).unwrap();
        let opened = open(&root.join("com.studio.pebble")).unwrap();
        assert!(opened.manifest.contains("com.studio.pebble"));
        let idle = opened.strips.iter().find(|s| s.name == "idle").unwrap();
        assert_eq!((idle.width, idle.height), (96, 48));
        assert_eq!(idle.pixels, pixels);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn opening_a_pet_that_is_not_48_pixels_is_refused_in_plain_words() {
        use super::super::studio::open;
        let root = root("refuse");
        save(
            &root,
            &manifest("com.studio.pebble", "Pebble"),
            &[image("idle", 2), image("move", 1)],
            &image("preview", 1),
        )
        .unwrap();
        let error = open(&root.join("com.studio.pebble")).unwrap_err();
        assert!(
            error.contains("48×48") && error.contains("32×32"),
            "{error}"
        );
        assert!(error.contains("Edit its files directly"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn every_bundled_pet_opens_in_studio_with_all_its_strips() {
        use super::super::{builtin, studio::open};
        let root = root("builtin");
        builtin::restore(&root, &[]).unwrap();
        for listing in super::super::loader::scan(&root) {
            let opened = open(Path::new(&listing.summary.directory)).unwrap();
            assert_eq!(opened.strips.len(), listing.summary.animations.len());
            for strip in &opened.strips {
                assert_eq!(strip.height, 48);
                assert_eq!(strip.width % 48, 0);
            }
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn saving_again_replaces_the_earlier_save() {
        let root = root("again");
        save(
            &root,
            &manifest("com.studio.pebble", "Pebble"),
            &[image("idle", 2), image("move", 1)],
            &image("preview", 1),
        )
        .unwrap();
        let summary = save(
            &root,
            &manifest("com.studio.pebble", "Pebble Two"),
            &[image("idle", 2), image("move", 1)],
            &image("preview", 1),
        )
        .unwrap();
        assert_eq!(summary.name, "Pebble Two");
        let listed: Vec<_> = super::super::loader::scan(&root)
            .into_iter()
            .filter(|l| l.summary.id == "com.studio.pebble")
            .collect();
        assert_eq!(listed.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_design_the_validator_rejects_is_explained_and_not_installed() {
        let root = root("invalid");
        let broken = manifest("com.studio.pebble", "Pebble")
            .replace(r#""to": "rest""#, r#""to": "nowhere""#);
        let error = save(
            &root,
            &broken,
            &[image("idle", 2), image("move", 1)],
            &image("preview", 1),
        )
        .unwrap_err();
        assert!(error.contains("could not be saved"), "{error}");
        assert!(error.contains("nowhere"), "{error}");
        assert!(!root.join("com.studio.pebble").exists());
        assert!(!root.join(".studio-staging").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn strange_names_and_short_pixel_buffers_are_refused_before_writing() {
        let root = root("strange");
        let mut short = image("idle", 2);
        short.pixels.truncate(10);
        assert!(save(
            &root,
            &manifest("com.studio.pebble", "Pebble"),
            &[short, image("move", 1)],
            &image("preview", 1)
        )
        .is_err());
        assert!(save(
            &root,
            &manifest("com.studio.pebble", "Pebble"),
            &[image("../escape", 1)],
            &image("preview", 1)
        )
        .is_err());
        assert!(!root.join(".studio-staging").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}

// ------------------------------------------------------------ throw and petting

#[test]
fn thrown_landed_hard_and_petted_parse_as_reaction_events() {
    for event in ["thrown", "landed_hard", "petted"] {
        let text = MINIMAL.replace(
            "\"animations\"",
            &format!(
                r#""behavior": {{
    "initial": "perch",
    "states": {{ "perch": {{ "transitions": [] }}, "wobble": {{ "transitions": [] }} }}
  }},
  "reactions": [{{ "on": "{event}", "goto": "wobble" }}],
  "animations""#
            ),
        );
        let (manifest, report) = parse(&text);
        assert!(!report.has_errors(), "{event}: {}", report.text());
        assert_eq!(manifest.unwrap().reactions[0].on.as_str(), event);
    }
}

#[test]
fn dizzy_and_petted_fall_back_to_what_the_pack_has() {
    let (manifest, _) = parse(MINIMAL);
    let mut manifest = manifest.unwrap();
    assert_eq!(manifest.resolve_clip("dizzy"), "idle");
    assert_eq!(manifest.resolve_clip("petted"), "idle");
    let clip = manifest.animations["idle"].clone();
    manifest.animations.insert("alert".into(), clip.clone());
    assert_eq!(manifest.resolve_clip("dizzy"), "alert");
    manifest.animations.insert("happy".into(), clip);
    assert_eq!(manifest.resolve_clip("petted"), "happy");
}

#[test]
fn a_pack_with_no_behavior_still_reacts_to_throws_and_petting() {
    let (manifest, _) = parse(MINIMAL);
    let manifest = manifest.unwrap();
    let defaults = manifest::default_reactions(&manifest.behavior);
    for event in ["thrown", "landed_hard", "petted"] {
        assert!(
            defaults.iter().any(|r| r.on.as_str() == event),
            "default reactions include {event}"
        );
    }
}

#[test]
fn every_bundled_pet_reacts_to_throws_and_petting_with_its_own_clips() {
    for name in ["budgie-classic", "squirrel-red", "gecko-glass", "cat-tabby"] {
        let pack = loader::load(&repo_pack(name), true).unwrap();
        for event in ["thrown", "landed_hard", "petted"] {
            assert!(
                pack.manifest
                    .reactions
                    .iter()
                    .any(|r| r.on.as_str() == event),
                "{name} reacts to {event}"
            );
        }
        assert!(pack.strips.contains_key("dizzy"), "{name} draws dizzy");
        assert!(pack.strips.contains_key("petted"), "{name} draws petted");
        assert!(
            pack.report
                .diagnostics
                .iter()
                .all(|d| d.tier != Tier::Error && d.tier != Tier::Warning),
            "{name}: {}",
            pack.report.text()
        );
    }
}

// ----------------------------------------------------------------------- the cat

#[test]
fn four_pets_ship_and_all_load_from_the_embedded_files_without_errors_or_warnings() {
    assert_eq!(
        super::builtin::BUILTIN.len(),
        5,
        "four pets and the meadow decor"
    );
    let root = std::env::temp_dir().join(format!("perch-builtin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let written = super::builtin::restore(&root, &[]).unwrap();
    assert!(written.contains(&"cat-tabby".to_string()), "{written:?}");
    assert!(root.join("cat-tabby/pet.json").is_file());
    let listings = loader::scan(&root);
    assert_eq!(listings.len(), 4);
    for listing in listings {
        assert!(listing.summary.loadable, "{}", listing.summary.name);
        assert_eq!(listing.summary.errors, 0, "{}", listing.report.text());
        assert_eq!(listing.summary.warnings, 0, "{}", listing.report.text());
    }
    let (decor, broken) = super::decor::scan(&root);
    assert_eq!(decor.len(), 1, "the meadow is decor, not a broken pet");
    assert!(
        broken.is_empty(),
        "{:?}",
        broken.iter().map(Report::text).collect::<Vec<_>>()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_cat_is_a_hopper_that_stays_on_floors_and_hops_140() {
    let cat = loader::load(&repo_pack("cat-tabby"), true).unwrap();
    assert_eq!(cat.manifest.name, "Miso the Cat");
    assert_eq!(cat.manifest.locomotion.class, LocomotionClass::Hopper);
    assert_eq!(cat.manifest.locomotion.jump.max_gap, 140.);
    assert_eq!(cat.manifest.locomotion.jump.arc_height, 60.);
    for kind in cat.manifest.locomotion.surfaces.iter() {
        assert_eq!(kind.normal().y, -1., "{kind:?} is somewhere to stand");
    }
    // `stretch` is optional: a pack without it plays idle.
    assert_eq!(cat.manifest.resolve_clip("stretch"), "stretch");
    let (bare, _) = parse(MINIMAL);
    assert_eq!(bare.unwrap().resolve_clip("stretch"), "idle");
}

// ------------------------------------------------------------------- pet distance

#[test]
fn pet_distance_is_a_condition_and_is_false_when_the_pet_is_alone() {
    use super::expr::{Condition, Signals};
    let near = Condition::parse("pet_distance < 120").unwrap();
    let alone = Signals::default();
    assert!(alone.pet_distance.is_infinite());
    assert!(!near.evaluate(&alone));
    assert!(near.evaluate(&Signals {
        pet_distance: 100.,
        ..alone
    }));
    let compound = Condition::parse("pet_distance < 120 && energy < 0.4").unwrap();
    assert!(compound.evaluate(&Signals {
        pet_distance: 100.,
        energy: 0.2,
        ..alone
    }));
    assert!(!compound.evaluate(&Signals {
        pet_distance: 100.,
        energy: 0.9,
        ..alone
    }));
}

#[test]
fn an_unknown_signal_still_lists_the_valid_ones_including_pet_distance() {
    let error = super::expr::Condition::parse("friend_distance < 3").unwrap_err();
    assert!(
        error.contains("Unknown signal `friend_distance`"),
        "{error}"
    );
    assert!(error.contains("pet_distance"), "{error}");
}

#[test]
fn pet_nearby_and_move_toward_pet_parse_and_every_bundled_pet_uses_them() {
    for name in ["budgie-classic", "squirrel-red", "gecko-glass", "cat-tabby"] {
        let pack = loader::load(&repo_pack(name), true).unwrap();
        assert!(
            pack.manifest
                .reactions
                .iter()
                .any(|r| r.on.as_str() == "pet_nearby" && r.cooldown >= 120.),
            "{name} greets a neighbour, rarely"
        );
        assert!(
            pack.manifest
                .behavior
                .states
                .values()
                .any(|s| s.action == Some(manifest::Action::MoveTowardPet)),
            "{name} can visit another pet"
        );
    }
}

/// Hostile and gallery-bound input: archive limits, the file allow-list,
/// atomic installs and updates, canonical export, and the publish rules.
mod hardening {
    use super::super::{
        builtin, limits,
        loader::{self, InstallMode},
        manifest::{self, Event, Version},
        sprites, Report, Signal,
    };
    use super::{errors, parse, repo_pack, MINIMAL};
    use std::io::Write;
    use std::path::{Path, PathBuf};

    fn scratch(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("perch-hard-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().flatten() {
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    /// A bundled pack copied somewhere it can be edited.
    fn editable(name: &str, root: &Path) -> PathBuf {
        let target = root.join(name);
        copy_tree(&repo_pack(name), &target);
        target
    }

    fn zip_of(path: &Path, files: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options: zip::write::FileOptions<()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    fn install_error(files: &[(&str, &[u8])], tag: &str) -> String {
        let root = scratch(tag);
        let archive = root.join("x.petpack");
        zip_of(&archive, files);
        let error = loader::install(&archive, &root).unwrap_err().to_string();
        let _ = std::fs::remove_dir_all(&root);
        error
    }

    fn gecko_manifest() -> Vec<u8> {
        std::fs::read(repo_pack("gecko-glass").join("pet.json")).unwrap()
    }

    #[test]
    fn an_archive_with_too_many_entries_is_refused_with_the_limit() {
        let names: Vec<String> = (0..=limits::MAX_ARCHIVE_ENTRIES)
            .map(|i| format!("sprites/s{i}.png"))
            .collect();
        let mut files: Vec<(&str, &[u8])> = vec![("pet.json", b"{}")];
        files.extend(names.iter().map(|n| (n.as_str(), &b"x"[..])));
        let error = install_error(&files, "entries");
        assert!(error.contains("entries"), "{error}");
        assert!(
            error.contains(&limits::MAX_ARCHIVE_ENTRIES.to_string()),
            "{error}"
        );
    }

    #[test]
    fn a_zip_bomb_is_stopped_by_the_per_file_and_total_limits() {
        let big = vec![0_u8; (limits::MAX_ENTRY_BYTES + 1) as usize];
        let error = install_error(&[("pet.json", b"{}"), ("sprites/a.png", &big)], "bomb-one");
        assert!(error.contains("MiB"), "{error}");
        // Each file is under the cap; together they are over the total.
        let chunk = vec![0_u8; (limits::MAX_ENTRY_BYTES - 1) as usize];
        let error = install_error(
            &[
                ("pet.json", b"{}"),
                ("sprites/a.png", &chunk),
                ("sprites/b.png", &chunk),
                ("sprites/c.png", &chunk),
                ("sprites/d.png", &chunk),
                ("sprites/e.png", &chunk),
            ],
            "bomb-total",
        );
        assert!(error.contains("expands to more than"), "{error}");
    }

    #[test]
    fn archive_entries_that_escape_or_do_not_belong_are_refused_by_name() {
        let error = install_error(&[("pet.json", b"{}"), ("../evil.txt", b"x")], "escape");
        assert!(error.contains("outside the pack folder"), "{error}");
        let error = install_error(&[("pet.json", b"{}"), ("run.sh", b"x")], "stray");
        assert!(error.contains("`run.sh` is not allowed"), "{error}");
        let error = install_error(&[("pet.json", b"{}"), ("sprites/Idle.PNG", b"x")], "case");
        assert!(error.contains("not allowed"), "{error}");
        let error = install_error(&[("readme.md", b"x")], "no-manifest");
        assert!(error.contains("no pet.json"), "{error}");
    }

    #[test]
    fn archive_symlinks_are_refused() {
        let root = scratch("symlink");
        let archive = root.join("x.petpack");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        let plain: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.start_file("pet.json", plain).unwrap();
        zip.write_all(b"{}").unwrap();
        zip.add_symlink("sprites/idle.png", "/etc/passwd", plain)
            .unwrap();
        zip.finish().unwrap();
        let error = loader::install(&archive, &root).unwrap_err().to_string();
        assert!(error.contains("symbolic link"), "{error}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_wrapping_folder_and_os_litter_in_an_archive_are_tolerated() {
        let root = scratch("wrapped");
        let archive = root.join("x.petpack");
        let gecko = repo_pack("gecko-glass");
        let manifest = gecko_manifest();
        let sprite_names: Vec<String> = std::fs::read_dir(gecko.join("sprites"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        let sprite_bytes: Vec<Vec<u8>> = sprite_names
            .iter()
            .map(|n| std::fs::read(gecko.join("sprites").join(n)).unwrap())
            .collect();
        let paths: Vec<String> = sprite_names
            .iter()
            .map(|n| format!("glass/sprites/{n}"))
            .collect();
        let mut files: Vec<(&str, &[u8])> = vec![
            ("glass/pet.json", &manifest),
            ("__MACOSX/glass/._pet.json", b"x"),
            ("glass/.DS_Store", b"x"),
        ];
        files.extend(
            paths
                .iter()
                .zip(&sprite_bytes)
                .map(|(p, b)| (p.as_str(), b.as_slice())),
        );
        zip_of(&archive, &files);
        let installed = loader::install(&archive, &root).expect("wrapped archive installs");
        assert_eq!(installed.id, "com.perch.gecko-glass");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_huge_png_is_refused_from_its_header_before_it_is_decoded() {
        let root = scratch("png");
        let path = root.join("wide.png");
        let (width, height) = (limits::MAX_PNG_WIDTH + 1, 4);
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer
                .write_image_data(&vec![0; (width * height * 4) as usize])
                .unwrap();
        }
        std::fs::write(&path, bytes).unwrap();
        let mut report = Report::default();
        assert!(sprites::load(&path, "sprites/wide.png", 1, &mut report).is_none());
        let found = errors(&report).join("\n");
        assert!(found.contains("at most"), "{found}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn behaviour_bounds_name_the_path_and_the_limit() {
        let transitions: Vec<String> = (0..=limits::MAX_TRANSITIONS_PER_STATE)
            .map(|_| r#"{ "to": "rest" }"#.to_string())
            .collect();
        let text = MINIMAL.replace(
            r#""animations": {"#,
            &format!(
                r#""behavior": {{ "initial": "rest", "states": {{ "rest": {{ "transitions": [{}] }} }} }},
  "animations": {{"#,
                transitions.join(",")
            ),
        );
        let (_, report) = parse(&text);
        let found = errors(&report).join("\n");
        assert!(
            found.contains("behavior.states.rest.transitions"),
            "{found}"
        );
        let tags = (0..=limits::MAX_TAGS)
            .map(|i| format!("\"t{i}\""))
            .collect::<Vec<_>>();
        let text = MINIMAL.replace(
            r#""animations": {"#,
            &format!(r#""tags": [{}], "animations": {{"#, tags.join(",")),
        );
        let (_, report) = parse(&text);
        assert!(errors(&report).join("\n").contains("tags"), "tags");
        let text = MINIMAL.replace("\"frames\": 4", "\"frames\": 65");
        let (_, report) = parse(&text);
        assert!(
            errors(&report)
                .join("\n")
                .contains("animations.idle.frames"),
            "frames"
        );
        let text = MINIMAL.replace("\"fps\": 6", "\"fps\": 61");
        let (_, report) = parse(&text);
        assert!(
            errors(&report).join("\n").contains("animations.idle.fps"),
            "fps"
        );
    }

    #[test]
    fn versions_order_numerically_and_reject_anything_else() {
        assert!(Version::parse("1.10.0") > Version::parse("1.9.9"));
        assert_eq!(Version::parse("1.0"), None);
        assert_eq!(Version::parse("1.0.0.0"), None);
        assert_eq!(Version::parse("1.0.x"), None);
    }

    #[test]
    fn an_update_replaces_a_lower_version_and_nothing_else() {
        let root = scratch("update");
        let packs = root.join("packs");
        std::fs::create_dir_all(&packs).unwrap();
        let work = editable("gecko-glass", &root);
        loader::install(&work, &packs).expect("first install");

        let error = loader::install_with(&work, &packs, InstallMode::Update)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("higher"),
            "same version is not an update: {error}"
        );
        let error = loader::install(&work, &packs).unwrap_err().to_string();
        assert!(error.contains("already installed"), "{error}");

        let manifest = std::fs::read_to_string(work.join("pet.json")).unwrap();
        std::fs::write(
            work.join("pet.json"),
            manifest.replace("\"version\": \"1.0.0\"", "\"version\": \"1.1.0\""),
        )
        .unwrap();
        let updated = loader::install_with(&work, &packs, InstallMode::Update).expect("update");
        assert_eq!(updated.version, "1.1.0");
        assert_eq!(
            loader::scan(&packs).len(),
            1,
            "updated in place, not duplicated"
        );

        std::fs::write(
            work.join("pet.json"),
            manifest.replace("\"version\": \"1.0.0\"", "\"version\": \"0.9.0\""),
        )
        .unwrap();
        let error = loader::install_with(&work, &packs, InstallMode::Update)
            .unwrap_err()
            .to_string();
        assert!(error.contains("higher"), "{error}");
        let still = loader::scan(&packs);
        assert_eq!(
            still[0].summary.version, "1.1.0",
            "a refused update leaves the pack alone"
        );
        assert!(still[0].summary.loadable);
        let leftovers: Vec<_> = std::fs::read_dir(&packs)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with(".incoming") || n.starts_with(".old"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");

        let error = loader::install_with(
            &repo_pack("cat-tabby"),
            &root.join("empty"),
            InstallMode::Update,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("nothing to update"), "{error}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_failed_install_never_damages_what_was_installed() {
        let root = scratch("atomic");
        let packs = root.join("packs");
        std::fs::create_dir_all(&packs).unwrap();
        loader::install(&repo_pack("gecko-glass"), &packs).unwrap();
        // A pack that fails validation never reaches the swap.
        let broken = editable("cat-tabby", &root);
        std::fs::write(broken.join("pet.json"), "{ not json").unwrap();
        assert!(loader::install(&broken, &packs).is_err());
        let listing = loader::scan(&packs);
        assert_eq!(listing.len(), 1);
        assert!(listing[0].summary.loadable);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn export_is_canonical_so_the_same_pack_always_hashes_the_same() {
        let root = scratch("canonical");
        let work = editable("gecko-glass", &root);
        std::fs::write(work.join("notes.txt"), "stray").unwrap();
        let first = loader::export(&work, &root.join("a.petpack")).unwrap();
        // Same content, different modification times.
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);
        for relative in ["pet.json", "preview.png"] {
            std::fs::File::options()
                .write(true)
                .open(work.join(relative))
                .unwrap()
                .set_modified(later)
                .unwrap();
        }
        let second = loader::export(&work, &root.join("b.petpack")).unwrap();
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(
            std::fs::read(root.join("a.petpack")).unwrap(),
            std::fs::read(root.join("b.petpack")).unwrap()
        );
        assert_eq!(
            loader::hash_petpack(&root.join("a.petpack")).unwrap(),
            first.sha256
        );
        assert_eq!(first.sha256.len(), 64);

        let mut zip =
            zip::ZipArchive::new(std::fs::File::open(root.join("a.petpack")).unwrap()).unwrap();
        let names: Vec<String> = zip.file_names().map(String::from).collect();
        assert!(!names.iter().any(|n| n == "notes.txt"), "{names:?}");
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "entries are sorted");
        assert!(names.iter().all(|n| loader::is_allowed_path(n)));
        let entry = zip.by_index(0).unwrap();
        assert_eq!(entry.last_modified().unwrap().year(), 1980);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn every_bundled_pack_passes_the_publish_rules() {
        for name in ["budgie-classic", "cat-tabby", "gecko-glass", "squirrel-red"] {
            let report = loader::validate_for_publish(&repo_pack(name));
            assert!(errors(&report).is_empty(), "{name}: {:?}", errors(&report));
        }
    }

    #[test]
    fn each_publish_rule_fails_with_its_own_message() {
        let root = scratch("publish");
        let check = |tag: &str, edit: &dyn Fn(&Path)| -> String {
            let work = editable("gecko-glass", &root.join(tag));
            edit(&work);
            errors(&loader::validate_for_publish(&work)).join("\n")
        };
        let rewrite = |from: &'static str, to: &'static str| {
            move |dir: &Path| {
                let text = std::fs::read_to_string(dir.join("pet.json")).unwrap();
                assert!(text.contains(from), "fixture lacks {from}");
                std::fs::write(dir.join("pet.json"), text.replace(from, to)).unwrap();
            }
        };
        assert!(check("licence", &rewrite("\"CC-BY-4.0\"", "\"Beerware\"")).contains("licence"));
        assert!(check("version", &rewrite("\"1.0.0\"", "\"1.0\"")).contains("version"));
        assert!(check("id", &rewrite("com.perch.gecko-glass", "Gecko")).contains("reverse-DNS"));
        assert!(check("size", &rewrite("\"width\": 48", "\"width\": 32")).contains("sprite.width"));
        assert!(check("no-preview", &|d| std::fs::remove_file(
            d.join("preview.png")
        )
        .unwrap())
        .contains("preview.png"));
        assert!(check("no-licence-file", &|d| std::fs::remove_file(
            d.join("LICENSE.txt")
        )
        .unwrap())
        .contains("LICENSE.txt"));
        assert!(
            check("stray", &|d| std::fs::write(d.join("run.sh"), "x").unwrap()).contains("run.sh")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn install_errors_carry_a_stable_code_the_ui_can_switch_on() {
        let root = scratch("codes");
        let packs = root.join("packs");
        std::fs::create_dir_all(&packs).unwrap();
        let work = editable("gecko-glass", &root);
        loader::install(&work, &packs).unwrap();
        let code = |r: Result<_, loader::InstallError>| -> &'static str {
            r.map(|_: loader::PackSummary| ()).unwrap_err().code
        };
        assert_eq!(code(loader::install(&work, &packs)), "already_installed");
        assert_eq!(
            code(loader::install_with(&work, &packs, InstallMode::Update)),
            "not_newer"
        );
        assert_eq!(
            code(loader::install_with(
                &repo_pack("cat-tabby"),
                &root.join("none"),
                InstallMode::Update
            )),
            "not_installed"
        );
        let broken = editable("cat-tabby", &root);
        std::fs::write(broken.join("pet.json"), "{ not json").unwrap();
        assert_eq!(code(loader::install(&broken, &packs)), "invalid_pack");
        let big = vec![0_u8; (limits::MAX_ENTRY_BYTES + 1) as usize];
        let archive = root.join("big.petpack");
        zip_of(&archive, &[("pet.json", b"{}"), ("sprites/a.png", &big)]);
        assert_eq!(code(loader::install(&archive, &packs)), "archive_limit");
        zip_of(&archive, &[("pet.json", b"{}"), ("run.sh", b"x")]);
        assert_eq!(code(loader::install(&archive, &packs)), "not_allowed");
        zip_of(&archive, &[("readme.md", b"x")]);
        assert_eq!(code(loader::install(&archive, &packs)), "no_manifest");
        std::fs::write(&archive, b"not a zip").unwrap();
        assert_eq!(
            code(loader::install(&archive, &packs)),
            "unreadable_archive"
        );
        let exported = loader::export(&work, &root.join("ok.petpack")).unwrap();
        assert!(loader::verify_hash(&root.join("ok.petpack"), &exported.sha256).is_ok());
        assert_eq!(
            loader::verify_hash(&root.join("ok.petpack"), &"0".repeat(64))
                .unwrap_err()
                .code,
            "hash_mismatch"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn published_names_are_append_only() {
        // Renaming or removing any of these breaks packs already in the wild.
        // Adding a name is safe: add it here in the same change.
        let events = [
            "surface_lost",
            "surface_moved_fast",
            "grabbed",
            "released",
            "clicked",
            "cursor_nearby",
            "cursor_fast",
            "user_idle_long",
            "user_returned",
            "long_session",
            "break_taken",
            "app_category_changed",
            "hour_changed",
            "startup",
            "low_energy",
            "thrown",
            "landed_hard",
            "petted",
            "pet_nearby",
            "new_day",
            "landed",
            "edge_reached",
        ];
        for name in events {
            assert!(
                Event::parse(name).is_some(),
                "event `{name}` was removed or renamed"
            );
        }
        assert_eq!(
            Event::ALL.len(),
            events.len(),
            "new event: add it to this list"
        );
        let signals = [
            "idle_seconds",
            "session_minutes",
            "app_category",
            "hour",
            "energy",
            "mood",
            "surface_count",
            "cursor_distance",
            "cursor_speed",
            "is_dragging",
            "pet_distance",
            "affection",
            "days_together",
        ];
        let live: Vec<&str> = Signal::ALL.iter().map(|s| s.as_str()).collect();
        for name in signals {
            assert!(
                live.contains(&name),
                "signal `{name}` was removed or renamed"
            );
        }
        assert_eq!(live.len(), signals.len(), "new signal: add it to this list");
        let actions = [
            "move_to_random_surface",
            "move_to_nearest_surface",
            "move_toward_cursor",
            "flee_from_cursor",
            "patrol_current_surface",
            "return_to_home_surface",
            "move_toward_pet",
        ];
        let live: Vec<&str> = manifest::Action::ALL.iter().map(|a| a.as_str()).collect();
        for name in actions {
            assert!(
                live.contains(&name),
                "action `{name}` was removed or renamed"
            );
        }
        assert_eq!(live.len(), actions.len(), "new action: add it to this list");
        let classes: Vec<&str> = manifest::LocomotionClass::ALL
            .iter()
            .map(|c| c.as_str())
            .collect();
        for name in ["flier", "climber", "walker", "hopper"] {
            assert!(
                classes.contains(&name),
                "class `{name}` was removed or renamed"
            );
        }
    }

    #[test]
    fn the_json_schema_carries_the_same_limits_as_the_code() {
        let schema: serde_json::Value = serde_json::from_str(builtin::SCHEMA).unwrap();
        let p = &schema["properties"];
        assert_eq!(p["tags"]["maxItems"], limits::MAX_TAGS);
        assert_eq!(p["description"]["maxLength"], limits::MAX_DESCRIPTION_CHARS);
        assert_eq!(p["animations"]["maxProperties"], limits::MAX_ANIMATIONS);
        assert_eq!(
            p["animations"]["additionalProperties"]["properties"]["frames"]["maximum"],
            limits::MAX_FRAMES
        );
        assert_eq!(
            p["animations"]["additionalProperties"]["properties"]["fps"]["maximum"],
            limits::MAX_FPS
        );
        assert_eq!(
            p["behavior"]["properties"]["states"]["maxProperties"],
            limits::MAX_STATES
        );
        assert_eq!(p["reactions"]["maxItems"], limits::MAX_REACTIONS);
        assert_eq!(
            p["behavior"]["properties"]["states"]["additionalProperties"]["properties"]
                ["transitions"]["maxItems"],
            limits::MAX_TRANSITIONS_PER_STATE
        );
    }
}

#[test]
fn a_pack_without_a_jelly_block_still_squishes_with_the_defaults() {
    let (manifest, report) = parse(MINIMAL);
    let manifest = manifest.unwrap();
    assert!(!report.has_errors(), "{}", report.text());
    assert_eq!(
        manifest.jelly_params(),
        crate::jelly::JellyParams::default()
    );
}

#[test]
fn a_jelly_block_tunes_the_springs_and_names_every_bad_value() {
    let with = |jelly: &str| {
        MINIMAL.replacen(
            "\"format\": 1,",
            &format!("\"format\": 1, \"jelly\": {jelly},"),
            1,
        )
    };
    let (manifest, report) = parse(&with(r#"{ "grip": 0.95, "squash": 1.5 }"#));
    let params = manifest.unwrap().jelly_params();
    assert!(!report.has_errors(), "{}", report.text());
    assert_eq!(params.grip, 0.95);
    assert_eq!(params.squash, 1.5);

    let (manifest, report) = parse(&with(r#"{ "grip": 4, "stiffness": 0, "wobble": 1 }"#));
    let params = manifest
        .expect("bad tuning never stops a pack loading")
        .jelly_params();
    assert_eq!(params.grip, 1.);
    assert!(params.stiffness > 0.);
    let paths: Vec<&str> = report.diagnostics.iter().map(|d| d.path.as_str()).collect();
    for path in ["jelly.grip", "jelly.stiffness", "jelly.wobble"] {
        assert!(paths.contains(&path), "{path} is named: {}", report.text());
    }
    let grip = report
        .diagnostics
        .iter()
        .find(|d| d.path == "jelly.grip")
        .unwrap();
    assert!(grip.expected.contains("0 to 1"), "{}", grip.expected);

    let (_, report) = parse(&with(r#"{ "grip": "tight" }"#));
    assert!(
        errors(&report).iter().any(|e| e.contains("jelly.grip")),
        "{}",
        report.text()
    );
}
