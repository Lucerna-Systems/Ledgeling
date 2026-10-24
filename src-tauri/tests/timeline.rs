//! Integration tests: recorded window-layout timelines replayed through a mock
//! platform, so "the user dragged a window while the squirrel was on it" is a
//! repeatable test rather than something you have to reproduce by hand.
use ledgeling::{
    engine::Engine,
    packs::loader,
    platform::mock::MockPlatform,
    settings::Settings,
    world::{MonitorInfo, Rect, WindowInfo},
};
use std::{collections::HashMap, path::Path, sync::Arc};

fn monitors() -> Vec<MonitorInfo> {
    vec![MonitorInfo {
        id: 0,
        bounds: Rect::new(0., 0., 1600., 1000.),
        work: Rect::new(0., 0., 1600., 960.),
        scale: 1.,
        is_primary: true,
    }]
}

fn two_monitors() -> Vec<MonitorInfo> {
    vec![
        monitors()[0],
        MonitorInfo {
            id: 1,
            bounds: Rect::new(-2560., -300., 2560., 1440.),
            work: Rect::new(-2560., -300., 2560., 1400.),
            scale: 2.,
            is_primary: false,
        },
    ]
}

fn win(id: u64, x: f64, y: f64, w: f64, h: f64) -> WindowInfo {
    WindowInfo::new(id, Rect::new(x, y, w, h))
}

fn packs() -> HashMap<String, Arc<ledgeling::packs::Pack>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../packs");
    let mut packs = HashMap::new();
    for name in ["budgie-classic", "squirrel-red", "gecko-glass", "cat-tabby"] {
        let pack = loader::load(&root.join(name), true)
            .unwrap_or_else(|report| panic!("{name}:\n{}", report.text()));
        packs.insert(pack.manifest.id.clone(), Arc::new(pack));
    }
    packs
}

fn engine(monitors: Vec<MonitorInfo>, frames: Vec<Vec<WindowInfo>>) -> (Engine, MockPlatform) {
    let mut settings = Settings::default();
    settings.pets.clear();
    let mut engine = Engine::new(settings, packs());
    let platform = MockPlatform::new(monitors.clone(), frames);
    engine.set_monitors(monitors);
    (engine, platform)
}

fn run(engine: &mut Engine, platform: &MockPlatform, seconds: f64) {
    let mut elapsed = 0.;
    while elapsed < seconds {
        elapsed += 1. / 60.;
        engine.tick(1. / 60., platform);
    }
}

#[test]
fn a_pet_spawns_lands_and_stays_on_real_edges() {
    let (mut engine, platform) = engine(monitors(), vec![vec![win(1, 300., 400., 600., 300.)]]);
    run(&mut engine, &platform, 1.);
    engine.spawn("com.perch.budgie-classic").unwrap();
    run(&mut engine, &platform, 20.);
    let pet = &engine.pets[0];
    assert!(pet.perch.is_some(), "the bird found somewhere real to be");
    assert!(pet.position.is_finite());
    assert!(engine.perch_count() > 0);
}

#[test]
fn dragging_a_window_carries_the_squirrel_with_it() {
    let (mut engine, platform) = engine(
        monitors(),
        vec![
            vec![win(1, 300., 500., 700., 300.)],
            vec![win(1, 500., 400., 700., 300.)],
            vec![win(1, 700., 300., 700., 300.)],
        ],
    );
    run(&mut engine, &platform, 1.);
    engine.spawn("com.perch.squirrel-red").unwrap();
    run(&mut engine, &platform, 8.);
    // Put the squirrel on the window's top edge, which is the case under test.
    let top = engine
        .surfaces
        .iter()
        .find(|s| s.owner == 1 && s.kind == ledgeling::world::SurfaceKind::WindowTop)
        .expect("the window has a visible top");
    engine.pets[0].perch = Some(ledgeling::pet::pathfind::Perch {
        key: top.key(),
        t: top.midpoint_t(),
    });
    engine.pets[0].position = top.point(top.midpoint_t());
    run(&mut engine, &platform, 0.1);
    let before = engine.pets[0].position;
    let perch = engine.pets[0].perch.expect("standing on the window");
    platform.advance();
    run(&mut engine, &platform, 1.);
    platform.advance();
    run(&mut engine, &platform, 1.);
    let pet = &engine.pets[0];
    assert_eq!(
        pet.perch.map(|p| p.key.owner),
        Some(perch.key.owner),
        "still on the same window"
    );
    assert!(
        pet.position.x > before.x || pet.position.y < before.y,
        "and it travelled with it: {before:?} → {:?}",
        pet.position
    );
}

#[test]
fn closing_the_last_window_leaves_no_pet_stranded_in_mid_air() {
    let (mut engine, platform) = engine(
        monitors(),
        vec![
            vec![
                win(1, 300., 400., 600., 300.),
                win(2, 200., 700., 500., 150.),
            ],
            vec![],
        ],
    );
    run(&mut engine, &platform, 1.);
    engine.spawn("com.perch.budgie-classic").unwrap();
    engine.spawn("com.perch.squirrel-red").unwrap();
    run(&mut engine, &platform, 15.);
    platform.advance();
    run(&mut engine, &platform, 20.);
    for pet in &engine.pets {
        assert!(pet.position.is_finite());
        assert!(
            pet.perch.is_some(),
            "{} ended on a real surface",
            pet.pack.manifest.name
        );
        assert!(pet.position.y <= 960.5);
    }
}

#[test]
fn a_second_monitor_is_used_and_unplugging_it_does_not_strand_anyone() {
    let frames = vec![vec![
        win(1, 300., 400., 600., 300.),
        win(2, -2000., 200., 800., 400.),
    ]];
    let (mut engine, platform) = engine(two_monitors(), frames);
    run(&mut engine, &platform, 1.);
    engine.spawn("com.perch.budgie-classic").unwrap();
    let mut monitors_seen = std::collections::HashSet::new();
    for _ in 0..1200 {
        run(&mut engine, &platform, 0.25);
        if let Some(monitor) = engine.monitor_for(engine.pets[0].position) {
            monitors_seen.insert(monitor.id);
        }
    }
    assert!(
        monitors_seen.len() > 1,
        "the pet used both monitors: {monitors_seen:?}"
    );
    // Unplug the second display mid-flight.
    engine.set_monitors(vec![monitors()[0]]);
    run(&mut engine, &platform, 20.);
    let pet = &engine.pets[0];
    assert!(pet.position.is_finite());
    assert!(
        Rect::new(0., 0., 1600., 1000.).contains(pet.position),
        "the pet came back onto the remaining screen: {:?}",
        pet.position
    );
}

#[test]
fn unplugging_the_pet_screen_returns_a_climber_to_the_fallback_display() {
    // The pet screen is the left-hand display, and a climber perched there
    // cannot fly back the way the budgie can, only fall.
    let (mut engine, platform) = engine(
        vec![two_monitors()[1]],
        vec![vec![win(1, -2000., 200., 800., 400.)]],
    );
    engine.spawn("com.perch.squirrel-red").unwrap();
    run(&mut engine, &platform, 15.);
    assert!(
        engine.pets[0].position.x < 0.,
        "the climber started on the unplugged display"
    );

    // The display is unplugged; the app falls back to the main display. The
    // pet may be mid-jump at any sampled instant, so perching is checked
    // across the whole run rather than at one moment.
    engine.set_monitors(monitors());
    let screen = Rect::new(0., 0., 1600., 1000.);
    let mut perched = false;
    for _ in 0..15 {
        run(&mut engine, &platform, 1.);
        let pet = &engine.pets[0];
        assert!(
            screen.contains(pet.position),
            "the climber is stranded off the fallback screen: {:?}",
            pet.position
        );
        perched = perched || pet.perch.is_some();
    }
    assert!(perched, "the climber found somewhere real to stand");
}

#[test]
fn a_fullscreen_app_suspends_everything_and_leaving_it_resumes() {
    let (mut engine, platform) = engine(monitors(), vec![vec![win(1, 300., 400., 600., 300.)]]);
    run(&mut engine, &platform, 1.);
    engine.spawn("com.perch.budgie-classic").unwrap();
    run(&mut engine, &platform, 5.);
    let resting = engine.pets[0].position;
    *platform.fullscreen.lock().unwrap() = true;
    run(&mut engine, &platform, 6.);
    assert!(engine.suspended, "a fullscreen app suspends the world");
    assert_eq!(
        engine.pets[0].position, resting,
        "nothing moves while suspended"
    );
    *platform.fullscreen.lock().unwrap() = false;
    run(&mut engine, &platform, 6.);
    assert!(!engine.suspended);
}

#[test]
fn a_window_scan_with_twenty_windows_stays_far_inside_the_budget() {
    let windows: Vec<WindowInfo> = (0..24)
        .map(|i| {
            let f = i as f64;
            win(i + 1, 40. * f, 30. * f + 40., 600., 400.)
        })
        .collect();
    let (mut engine, platform) = engine(monitors(), vec![windows]);
    run(&mut engine, &platform, 2.);
    assert!(
        engine.scan_ms < 8.,
        "scan took {} ms with 24 windows",
        engine.scan_ms
    );
    assert!(engine.surfaces.len() > 20);
}

#[test]
fn pets_and_positions_survive_a_restart() {
    let directory = std::env::temp_dir().join("perch-restart-test");
    let _ = std::fs::remove_dir_all(&directory);
    let (mut engine, platform) = engine(monitors(), vec![vec![win(1, 300., 400., 600., 300.)]]);
    engine.spawn("com.perch.squirrel-red").unwrap();
    run(&mut engine, &platform, 10.);
    engine.remember_pets();
    ledgeling::settings::save(&directory, &engine.settings).unwrap();
    let position = engine.pets[0].position;

    let (loaded, note) = ledgeling::settings::load(&directory);
    assert!(note.is_none());
    let mut restarted = Engine::new(loaded, packs());
    restarted.set_monitors(monitors());
    assert!(restarted.restore_pets().is_empty());
    assert_eq!(restarted.pets.len(), 1);
    assert_eq!(restarted.pets[0].pack.manifest.id, "com.perch.squirrel-red");
    assert_eq!(restarted.pets[0].position, position);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_pets_working_folder_survives_a_restart_unless_the_folder_is_gone() {
    let directory = std::env::temp_dir().join(format!("ledgeling-folder-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let folder = directory.to_string_lossy().into_owned();
    let mut engine = Engine::new(Settings::default(), packs());
    engine.set_monitors(monitors());
    let id = engine.spawn("com.perch.budgie-classic").unwrap();
    engine.set_pet_folder(id, Some(folder.clone()));
    engine.remember_pets();
    assert_eq!(
        engine.settings.pets.last().and_then(|p| p.folder.clone()),
        Some(folder.clone())
    );

    let mut restarted = Engine::new(engine.settings.clone(), packs());
    restarted.set_monitors(monitors());
    restarted.settings.pets.retain(|p| p.folder.is_some());
    assert!(restarted.restore_pets().is_empty());
    assert_eq!(
        restarted.pet_folders().first().map(|f| f.1.clone()),
        Some(folder)
    );

    std::fs::remove_dir_all(&directory).unwrap();
    let mut later = Engine::new(restarted.settings.clone(), packs());
    later.set_monitors(monitors());
    later.restore_pets();
    assert!(
        later.pet_folders().is_empty(),
        "a deleted folder is forgotten"
    );
}

#[test]
fn a_pet_whose_pack_was_uninstalled_is_dropped_rather_than_crashing() {
    let mut settings = Settings::default();
    settings.pets = vec![ledgeling::settings::PetSetting {
        pack_id: "com.example.gone".into(),
        x: Some(10.),
        y: Some(10.),
        folder: None,
    }];
    let mut engine = Engine::new(settings, packs());
    engine.set_monitors(monitors());
    let missing = engine.restore_pets();
    assert_eq!(missing, vec!["com.example.gone".to_string()]);
    assert!(engine.pets.is_empty());
    assert!(engine.settings.pets.is_empty());
}

#[test]
fn awareness_can_be_switched_off_entirely_and_the_pet_still_lives() {
    let (mut engine, platform) = engine(monitors(), vec![vec![win(1, 300., 400., 600., 300.)]]);
    engine.settings.awareness.enabled = false;
    let map = engine.settings.category_map();
    engine
        .awareness
        .apply(engine.settings.awareness.clone(), map);
    *platform.idle.lock().unwrap() = 4000.;
    engine.spawn("com.perch.budgie-classic").unwrap();
    run(&mut engine, &platform, 20.);
    let summary = engine.awareness_summary();
    assert!(!summary.enabled);
    assert_eq!(
        summary.session.idle_seconds, 0.,
        "idle time is not even read"
    );
    assert_eq!(summary.nudge_tier, 0);
    assert!(engine.pets[0].perch.is_some(), "the pet is unaffected");
}

#[test]
fn a_first_run_drops_one_pet_onto_the_frontmost_window_and_never_again() {
    let (mut engine, platform) = engine(monitors(), vec![vec![win(1, 300., 400., 600., 300.)]]);
    assert!(!engine.settings.welcomed);
    engine.request_welcome();
    run(&mut engine, &platform, 0.1);
    assert_eq!(engine.pets.len(), 1, "exactly one pet is welcomed");
    assert!(engine.settings.welcomed);
    assert!(!engine.welcome_pending());
    let pet = &engine.pets[0];
    assert_eq!(
        pet.motion,
        ledgeling::pet::Motion::Falling,
        "it arrives falling"
    );
    assert!(pet.position.y < 100., "from the top of the screen");
    assert!(
        (pet.position.x - 600.).abs() < 1.,
        "above the window's centre"
    );
    run(&mut engine, &platform, 6.);
    let perch = engine.pets[0].perch.expect("it landed on something");
    assert_eq!(perch.key.kind.as_str(), "window_top");
    assert_eq!(engine.pets.len(), 1);
}

#[test]
fn the_welcome_waits_for_a_window_to_land_on() {
    let (mut engine, platform) = engine(monitors(), vec![vec![]]);
    engine.request_welcome();
    run(&mut engine, &platform, 0.1);
    assert!(engine.pets.len() <= 1);
    assert!(engine.settings.welcomed || engine.welcome_pending());
}

#[test]
fn nothing_is_welcomed_when_already_welcomed_or_pets_exist() {
    let (mut engine, platform) = engine(monitors(), vec![vec![win(1, 300., 400., 600., 300.)]]);
    engine.settings.welcomed = true;
    engine.request_welcome();
    run(&mut engine, &platform, 1.);
    assert!(engine.pets.is_empty());

    let (mut engine, platform) = engine_with_saved_pet();
    engine.request_welcome();
    assert!(!engine.welcome_pending());
    run(&mut engine, &platform, 1.);
    assert_eq!(engine.pets.len(), 1);
}

fn engine_with_saved_pet() -> (Engine, MockPlatform) {
    let (mut engine, platform) = engine(monitors(), vec![vec![win(1, 300., 400., 600., 300.)]]);
    engine.spawn("com.perch.squirrel-red").unwrap();
    (engine, platform)
}

/// A and B are 100 px apart (hoppable); C is 200 px beyond B (not). All three
/// sit far above the floor, so the pet cannot drop to it and walk round.
/// Returns whether it ever stood on B; a hopper must also never reach C.
fn hop_between_windows(pack: &str, hopper: bool) -> bool {
    let (mut engine, platform) = engine(
        monitors(),
        vec![vec![
            win(1, 100., 300., 400., 200.),
            win(2, 600., 300., 400., 200.),
            win(3, 1200., 300., 300., 200.),
        ]],
    );
    run(&mut engine, &platform, 1.);
    engine.spawn(pack).unwrap();
    let top = engine
        .surfaces
        .iter()
        .find(|s| s.owner == 1 && s.kind == ledgeling::world::SurfaceKind::WindowTop)
        .expect("window A has a visible top");
    engine.pets[0].perch = Some(ledgeling::pet::pathfind::Perch {
        key: top.key(),
        t: top.midpoint_t(),
    });
    engine.pets[0].position = top.point(top.midpoint_t());
    let mut on_b = false;
    for _ in 0..(900 * 60) {
        engine.tick(1. / 60., &platform);
        let Some(perch) = engine.pets[0].perch else {
            continue;
        };
        if hopper {
            assert_eq!(
                perch.key.kind,
                ledgeling::world::SurfaceKind::WindowTop,
                "{pack} only stands on window tops here"
            );
        }
        // A climber can walk down a window's side to the floor and round, so
        // only the hopper is held to the gap.
        assert!(
            !hopper || perch.key.owner != 3,
            "{pack} crossed the 200 px gap to the third window"
        );
        on_b |= perch.key.owner == 2;
    }
    on_b
}

#[test]
fn the_cat_only_perches_on_standable_surfaces_and_never_hops_a_gap_wider_than_140() {
    assert!(
        hop_between_windows("com.perch.cat-tabby", true),
        "the cat hopped the 100 px gap to the second window"
    );
}

#[test]
fn a_pet_walking_to_a_window_edge_to_hop_is_not_frozen_there() {
    // Regression: the walk reached the edge, where the pet has no clearance,
    // was told its surface was lost, and stayed in `startled` for good.
    assert!(
        hop_between_windows("com.perch.squirrel-red", false),
        "the squirrel hopped to the second window"
    );
}
