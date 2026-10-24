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
    for name in ["budgie-classic", "squirrel-red", "gecko-glass"] {
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
    let error = loader::install(&repo_pack("gecko-glass"), &root).unwrap_err();
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
    use std::path::PathBuf;

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
        let root = root("refuse");
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
    for name in ["budgie-classic", "squirrel-red", "gecko-glass"] {
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
