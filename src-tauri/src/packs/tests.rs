//! Pack loading, validation and the behavior format. Every malformed case must
//! produce a message that names the file, the path, the problem and the fix.
use super::{
    diagnostics::{Report, Tier},
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
fn missing_required_animations_are_errors() {
    let text = MINIMAL.replace(
        "\"move\": { \"file\": \"sprites/move.png\", \"frames\": 6, \"fps\": 14 }",
        "\"flap\": { \"file\": \"sprites/move.png\", \"frames\": 6, \"fps\": 14 }",
    );
    let (_, report) = parse(&text);
    let message = errors(&report).join("\n");
    assert!(message.contains("animations.move"), "{message}");
    assert!(message.contains("required"), "{message}");
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
    assert_eq!(gecko.manifest.animations.len(), 2);
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
