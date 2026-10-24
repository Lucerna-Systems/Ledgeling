//! Typed reading of `pet.json`. The manifest is walked by hand rather than
//! derived, because every failure has to name the file, the JSON path, the
//! problem and the expectation — a derived deserializer cannot do that.
use super::{
    diagnostics::{Diagnostic, Report, Tier},
    expr::Condition,
};
use crate::world::SurfaceKind;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub const FORMAT_VERSION: u64 = 1;
pub const MANIFEST_FILE: &str = "pet.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocomotionClass {
    Flier,
    Climber,
    Walker,
    Hopper,
}
impl LocomotionClass {
    pub const ALL: [LocomotionClass; 4] = [
        LocomotionClass::Flier,
        LocomotionClass::Climber,
        LocomotionClass::Walker,
        LocomotionClass::Hopper,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            LocomotionClass::Flier => "flier",
            LocomotionClass::Climber => "climber",
            LocomotionClass::Walker => "walker",
            LocomotionClass::Hopper => "hopper",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == name)
    }
    pub fn falls(self) -> bool {
        !matches!(self, LocomotionClass::Flier)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AirStyle {
    Straight,
    FlapArc,
    Flutter,
    Glide,
}
impl AirStyle {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "straight" => AirStyle::Straight,
            "flap_arc" => AirStyle::FlapArc,
            "flutter" => AirStyle::Flutter,
            "glide" => AirStyle::Glide,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceOrientation {
    /// Rotate to match the surface: a squirrel flat against a window side.
    Align,
    /// Never rotate: a bird stays upright wherever it perches.
    Upright,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Facing {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpriteBlock {
    pub width: f64,
    pub height: f64,
    pub anchor_x: f64,
    pub anchor_y: f64,
    pub default_facing: Facing,
    pub scale: f64,
    pub surface_orientation: SurfaceOrientation,
}
impl Default for SpriteBlock {
    fn default() -> Self {
        Self {
            width: 48.,
            height: 48.,
            anchor_x: 0.5,
            anchor_y: 1.0,
            default_facing: Facing::Right,
            scale: 1.,
            surface_orientation: SurfaceOrientation::Upright,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Speed {
    pub surface: f64,
    pub air: f64,
    pub climb: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Jump {
    pub enabled: bool,
    pub max_gap: f64,
    pub arc_height: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Locomotion {
    pub class: LocomotionClass,
    pub surfaces: Vec<SurfaceKind>,
    pub speed: Speed,
    pub jump: Jump,
    pub gravity: f64,
    pub air_style: AirStyle,
    pub min_surface_length: f64,
}
impl Locomotion {
    pub fn defaults(class: LocomotionClass) -> Self {
        use LocomotionClass::*;
        let surfaces = match class {
            Flier => expand_surfaces(&["window_top", "monitor_top", "taskbar_top"]),
            Climber => expand_surfaces(&["window_any", "monitor_any", "taskbar_top"]),
            Walker => expand_surfaces(&["window_top", "taskbar_top"]),
            Hopper => expand_surfaces(&["window_top", "monitor_bottom", "taskbar_top"]),
        };
        Self {
            class,
            surfaces,
            speed: Speed {
                surface: match class {
                    Flier => 45.,
                    Climber => 110.,
                    Walker => 80.,
                    Hopper => 90.,
                },
                air: if class == Flier { 220. } else { 0. },
                climb: if class == Climber { 85. } else { 0. },
            },
            jump: Jump {
                enabled: matches!(class, Climber | Hopper),
                max_gap: match class {
                    Climber => 180.,
                    Hopper => 90.,
                    _ => 0.,
                },
                arc_height: 70.,
            },
            gravity: if class == Flier { 0. } else { 1. },
            air_style: if class == Flier {
                AirStyle::FlapArc
            } else {
                AirStyle::Straight
            },
            min_surface_length: 48.,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Animation {
    pub file: String,
    pub frames: u32,
    pub fps: f64,
    pub looping: bool,
    pub pingpong: bool,
    pub sound: Option<String>,
    pub offset_x: f64,
    pub offset_y: f64,
}

/// Clips fall back down a chain so a two-animation pack works everywhere.
pub const CLIP_FALLBACKS: &[(&str, &[&str])] = &[
    ("climb", &["move", "idle"]),
    ("jump", &["move", "idle"]),
    ("fall", &["move", "idle"]),
    ("land", &["idle"]),
    ("takeoff", &["move", "idle"]),
    ("sleep", &["idle"]),
    ("alert", &["idle"]),
    ("happy", &["idle"]),
    ("grabbed", &["idle"]),
    ("preen", &["idle"]),
    ("greet", &["happy", "idle"]),
    ("stretch", &["idle"]),
    ("dizzy", &["alert", "idle"]),
    ("petted", &["happy", "idle"]),
    // Assistant clips: what the pet plays while a task of its own is running,
    // and while that task is waiting for the user to answer something.
    // Not `move`: a walk cycle playing on a pet standing still reads as a bug.
    ("busy", &["preen", "idle"]),
    ("attention", &["alert", "happy", "idle"]),
];

/// Clips the assistant layer plays, and what a pack without them falls back to.
/// Packs are told about these rather than required to have them — the pack
/// format's promise is that a two-animation pack still works everywhere.
pub const ASSISTANT_CLIPS: &[(&str, &str)] = &[
    ("busy", "while this pet is working on a task"),
    ("attention", "while its task is waiting for your answer"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    MoveToRandomSurface,
    MoveToNearestSurface,
    MoveTowardCursor,
    FleeFromCursor,
    PatrolCurrentSurface,
    ReturnToHomeSurface,
    MoveTowardPet,
}
impl Action {
    pub const ALL: [Action; 7] = [
        Action::MoveToRandomSurface,
        Action::MoveToNearestSurface,
        Action::MoveTowardCursor,
        Action::FleeFromCursor,
        Action::PatrolCurrentSurface,
        Action::ReturnToHomeSurface,
        Action::MoveTowardPet,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Action::MoveToRandomSurface => "move_to_random_surface",
            Action::MoveToNearestSurface => "move_to_nearest_surface",
            Action::MoveTowardCursor => "move_toward_cursor",
            Action::FleeFromCursor => "flee_from_cursor",
            Action::PatrolCurrentSurface => "patrol_current_surface",
            Action::ReturnToHomeSurface => "return_to_home_surface",
            Action::MoveTowardPet => "move_toward_pet",
        }
    }
    fn parse(name: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.as_str() == name)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transition {
    pub to: String,
    pub weight: f64,
    pub when: Option<Condition>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub clip: String,
    pub duration: (f64, f64),
    pub action: Option<Action>,
    pub sound: Option<String>,
    pub transitions: Vec<Transition>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Behavior {
    pub initial: String,
    pub idle_wander_chance: f64,
    pub states: BTreeMap<String, State>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    SurfaceLost,
    SurfaceMovedFast,
    Grabbed,
    Released,
    Clicked,
    CursorNearby,
    CursorFast,
    UserIdleLong,
    UserReturned,
    LongSession,
    BreakTaken,
    AppCategoryChanged,
    HourChanged,
    Startup,
    LowEnergy,
    Thrown,
    LandedHard,
    Petted,
    PetNearby,
    NewDay,
    /// The pet came down onto a surface after falling.
    Landed,
    /// The pet walked to the end of a horizontal surface and stopped there.
    EdgeReached,
}
impl Event {
    pub const ALL: [Event; 22] = [
        Event::SurfaceLost,
        Event::SurfaceMovedFast,
        Event::Grabbed,
        Event::Released,
        Event::Clicked,
        Event::CursorNearby,
        Event::CursorFast,
        Event::UserIdleLong,
        Event::UserReturned,
        Event::LongSession,
        Event::BreakTaken,
        Event::AppCategoryChanged,
        Event::HourChanged,
        Event::Startup,
        Event::LowEnergy,
        Event::Thrown,
        Event::LandedHard,
        Event::Petted,
        Event::PetNearby,
        Event::NewDay,
        Event::Landed,
        Event::EdgeReached,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Event::SurfaceLost => "surface_lost",
            Event::SurfaceMovedFast => "surface_moved_fast",
            Event::Grabbed => "grabbed",
            Event::Released => "released",
            Event::Clicked => "clicked",
            Event::CursorNearby => "cursor_nearby",
            Event::CursorFast => "cursor_fast",
            Event::UserIdleLong => "user_idle_long",
            Event::UserReturned => "user_returned",
            Event::LongSession => "long_session",
            Event::BreakTaken => "break_taken",
            Event::AppCategoryChanged => "app_category_changed",
            Event::HourChanged => "hour_changed",
            Event::Startup => "startup",
            Event::LowEnergy => "low_energy",
            Event::Thrown => "thrown",
            Event::LandedHard => "landed_hard",
            Event::Petted => "petted",
            Event::PetNearby => "pet_nearby",
            Event::NewDay => "new_day",
            Event::Landed => "landed",
            Event::EdgeReached => "edge_reached",
        }
    }
    pub fn parse(name: &str) -> Option<Event> {
        Event::ALL.into_iter().find(|e| e.as_str() == name)
    }
    /// Priority bands from the awareness design; higher interrupts lower.
    pub fn default_priority(self) -> i32 {
        match self {
            Event::SurfaceLost | Event::SurfaceMovedFast => 100,
            Event::Grabbed
            | Event::Released
            | Event::Clicked
            | Event::Thrown
            | Event::LandedHard => 90,
            Event::UserReturned
            | Event::CursorNearby
            | Event::CursorFast
            | Event::Petted
            | Event::PetNearby => 60,
            Event::LongSession | Event::BreakTaken | Event::NewDay => 50,
            _ => 30,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reaction {
    pub on: Event,
    pub goto: String,
    pub priority: i32,
    pub cooldown: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Personality {
    pub activity: f64,
    pub boldness: f64,
    pub sociability: f64,
    pub sleepiness: f64,
    pub curiosity: f64,
}
impl Default for Personality {
    fn default() -> Self {
        Self {
            activity: 0.5,
            boldness: 0.5,
            sociability: 0.5,
            sleepiness: 0.5,
            curiosity: 0.5,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: u64,
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub license: String,
    pub tags: Vec<String>,
    pub sprite: SpriteBlock,
    pub locomotion: Locomotion,
    pub animations: BTreeMap<String, Animation>,
    pub behavior: Behavior,
    pub reactions: Vec<Reaction>,
    pub personality: Personality,
}

impl Manifest {
    /// The clip actually played for a requested name, following the fallback
    /// chain so a missing optional animation never breaks a pack.
    pub fn resolve_clip<'a>(&'a self, wanted: &'a str) -> &'a str {
        if self.animations.contains_key(wanted) {
            return wanted;
        }
        for (name, chain) in CLIP_FALLBACKS {
            if *name == wanted {
                for candidate in *chain {
                    if self.animations.contains_key(*candidate) {
                        return candidate;
                    }
                }
            }
        }
        // Idle and move cover for each other, and a pack with neither still
        // plays whatever it does have rather than failing to load.
        for candidate in ["idle", "move"] {
            if self.animations.contains_key(candidate) {
                return candidate;
            }
        }
        self.animations.keys().next().map_or("idle", String::as_str)
    }
    pub fn animation(&self, wanted: &str) -> Option<&Animation> {
        self.animations.get(self.resolve_clip(wanted))
    }
    pub fn allows(&self, kind: SurfaceKind) -> bool {
        self.locomotion.surfaces.contains(&kind)
    }
}

/// Parse a manifest, collecting every problem rather than stopping at the first.
/// Returns `None` only when the errors make the pack unloadable.
pub fn parse(text: &str, report: &mut Report) -> Option<Manifest> {
    let value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => {
            report.push(Diagnostic::error(
                MANIFEST_FILE,
                "",
                format!(
                    "The file is not valid JSON: {} at line {}, column {}.",
                    json_problem(&error),
                    error.line(),
                    error.column()
                ),
                "Check for a missing comma, a trailing comma, or an unclosed bracket.",
            ));
            return None;
        }
    };
    let root = expect_object(&value, "", report)?;
    let format = integer(root, "", "format", true, report).unwrap_or(FORMAT_VERSION);
    if format != FORMAT_VERSION {
        report.push(Diagnostic::error(
            MANIFEST_FILE,
            "format",
            format!("This pack declares format {format}."),
            format!("This build understands format {FORMAT_VERSION}."),
        ));
    }
    let id = string(root, "", "id", true, report)?;
    if !id.contains('.') {
        report.push(Diagnostic::warning(
            MANIFEST_FILE,
            "id",
            format!("`{id}` is not a reverse-DNS identifier."),
            "Use something like `com.yourname.simplebird` so ids stay unique.",
        ));
    }
    let name = string(root, "", "name", true, report)?;
    let author = string(root, "", "author", true, report)?;
    let version = string(root, "", "version", true, report)?;
    if !version.split('.').all(|p| p.parse::<u32>().is_ok()) || version.split('.').count() != 3 {
        report.push(Diagnostic::warning(
            MANIFEST_FILE,
            "version",
            format!("`{version}` is not a semver version."),
            "Use three numbers, like `1.0.0`.",
        ));
    }
    let sprite = read_sprite(root, report);
    let locomotion = read_locomotion(root, report)?;
    let animations = read_animations(root, report);
    let behavior = match root.get("behavior") {
        Some(value) => read_behavior(value, &animations, report),
        None => default_behavior(locomotion.class),
    };
    let reactions = read_reactions(root, &behavior, report);
    let personality = read_personality(root, report);
    note_unknown(
        root,
        "",
        &[
            "$schema",
            "format",
            "id",
            "name",
            "author",
            "version",
            "description",
            "license",
            "tags",
            "sprite",
            "locomotion",
            "animations",
            "behavior",
            "reactions",
            "personality",
        ],
        report,
    );
    Some(Manifest {
        format,
        id,
        name,
        author,
        version,
        description: string(root, "", "description", false, report).unwrap_or_default(),
        license: string(root, "", "license", false, report).unwrap_or_default(),
        tags: strings(root, "", "tags", report),
        sprite,
        locomotion,
        animations,
        behavior,
        reactions,
        personality,
    })
}

/// serde's own message is developer-facing; this is the author-facing version.
fn json_problem(error: &serde_json::Error) -> &'static str {
    use serde_json::error::Category;
    match error.classify() {
        Category::Io => "the file could not be read",
        Category::Syntax => "the JSON syntax is broken",
        Category::Data => "a value has the wrong shape",
        Category::Eof => "the file ends before the JSON is finished",
    }
}

fn read_sprite(root: &Map<String, Value>, report: &mut Report) -> SpriteBlock {
    let mut sprite = SpriteBlock::default();
    let Some(value) = root.get("sprite") else {
        report.push(Diagnostic::error(
            MANIFEST_FILE,
            "sprite",
            "The sprite block is missing.",
            "Add `\"sprite\": { \"width\": 48, \"height\": 48 }` describing one frame.",
        ));
        return sprite;
    };
    let Some(map) = expect_object(value, "sprite", report) else {
        return sprite;
    };
    if let Some(w) = number(map, "sprite", "width", true, report) {
        sprite.width = w;
    }
    if let Some(h) = number(map, "sprite", "height", true, report) {
        sprite.height = h;
    }
    for (field, value) in [("width", sprite.width), ("height", sprite.height)] {
        if value <= 0. {
            report.push(Diagnostic::error(
                MANIFEST_FILE,
                format!("sprite.{field}"),
                format!("Found {value}."),
                "Frame size must be a positive number of pixels.",
            ));
        }
    }
    if let Some(anchor) = map.get("anchor") {
        if let Some(anchor) = expect_object(anchor, "sprite.anchor", report) {
            sprite.anchor_x = number(anchor, "sprite.anchor", "x", false, report).unwrap_or(0.5);
            sprite.anchor_y = number(anchor, "sprite.anchor", "y", false, report).unwrap_or(1.0);
            note_unknown(anchor, "sprite.anchor", &["x", "y"], report);
        }
    }
    if let Some(facing) = string(map, "sprite", "default_facing", false, report) {
        match facing.as_str() {
            "left" => sprite.default_facing = Facing::Left,
            "right" => sprite.default_facing = Facing::Right,
            other => report.push(Diagnostic::error(
                MANIFEST_FILE,
                "sprite.default_facing",
                format!("Found `{other}`."),
                "Expected `left` or `right`.",
            )),
        }
    }
    if let Some(scale) = number(map, "sprite", "scale", false, report) {
        if scale > 0. {
            sprite.scale = scale;
        } else {
            report.push(Diagnostic::error(
                MANIFEST_FILE,
                "sprite.scale",
                format!("Found {scale}."),
                "Scale must be greater than zero.",
            ));
        }
    }
    if let Some(orientation) = string(map, "sprite", "surface_orientation", false, report) {
        match orientation.as_str() {
            "align" => sprite.surface_orientation = SurfaceOrientation::Align,
            "upright" => sprite.surface_orientation = SurfaceOrientation::Upright,
            other => report.push(Diagnostic::error(
                MANIFEST_FILE,
                "sprite.surface_orientation",
                format!("Found `{other}`."),
                "Expected `align` (rotate to the surface) or `upright` (never rotate).",
            )),
        }
    }
    note_unknown(
        map,
        "sprite",
        &[
            "width",
            "height",
            "anchor",
            "default_facing",
            "scale",
            "surface_orientation",
        ],
        report,
    );
    sprite
}

fn read_locomotion(root: &Map<String, Value>, report: &mut Report) -> Option<Locomotion> {
    let Some(value) = root.get("locomotion") else {
        report.push(Diagnostic::error(
            MANIFEST_FILE,
            "locomotion",
            "The locomotion block is missing.",
            "Add `\"locomotion\": { \"class\": \"flier\" }`. Classes: flier, climber, walker, hopper.",
        ));
        return None;
    };
    let map = expect_object(value, "locomotion", report)?;
    let class_name = string(map, "locomotion", "class", true, report)?;
    let Some(class) = LocomotionClass::parse(&class_name) else {
        report.push(Diagnostic::error(
            MANIFEST_FILE,
            "locomotion.class",
            format!("Found `{class_name}`."),
            format!(
                "Expected one of: {}.",
                LocomotionClass::ALL
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
        return None;
    };
    let mut locomotion = Locomotion::defaults(class);
    if let Some(list) = map.get("surfaces") {
        if let Some(items) = expect_array(list, "locomotion.surfaces", report) {
            let mut kinds = Vec::new();
            for (i, item) in items.iter().enumerate() {
                let path = format!("locomotion.surfaces[{i}]");
                let Some(name) = item.as_str() else {
                    report.push(Diagnostic::error(
                        MANIFEST_FILE,
                        path,
                        format!("Found {}.", describe(item)),
                        "Surface names are strings, such as \"window_top\".",
                    ));
                    continue;
                };
                let expanded = expand_surfaces(&[name]);
                if expanded.is_empty() {
                    report.push(Diagnostic::error(
                        MANIFEST_FILE,
                        path,
                        format!("Unknown surface `{name}`."),
                        format!(
                            "Expected one of: {}, or a group: window_any, window_vertical, monitor_any, all.",
                            SurfaceKind::ALL
                                .iter()
                                .map(|k| k.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    ));
                    continue;
                }
                kinds.extend(expanded);
            }
            kinds.sort_by_key(|k| k.as_str());
            kinds.dedup();
            if kinds.is_empty() {
                report.push(Diagnostic::error(
                    MANIFEST_FILE,
                    "locomotion.surfaces",
                    "No usable surfaces are listed.",
                    "A pet needs at least one surface, or the field can be omitted for the class default.",
                ));
            } else {
                locomotion.surfaces = kinds;
            }
        }
    }
    if let Some(speed) = map.get("speed") {
        if let Some(speed) = expect_object(speed, "locomotion.speed", report) {
            for (key, target) in [
                ("surface", &mut locomotion.speed.surface),
                ("air", &mut locomotion.speed.air),
                ("climb", &mut locomotion.speed.climb),
            ] {
                if let Some(v) = number(speed, "locomotion.speed", key, false, report) {
                    *target = v.max(0.);
                }
            }
            note_unknown(
                speed,
                "locomotion.speed",
                &["surface", "air", "climb"],
                report,
            );
        }
    }
    if let Some(jump) = map.get("jump") {
        if let Some(jump) = expect_object(jump, "locomotion.jump", report) {
            if let Some(enabled) = boolean(jump, "locomotion.jump", "enabled", false, report) {
                locomotion.jump.enabled = enabled;
            }
            if let Some(gap) = number(jump, "locomotion.jump", "max_gap", false, report) {
                locomotion.jump.max_gap = gap.max(0.);
            }
            if let Some(arc) = number(jump, "locomotion.jump", "arc_height", false, report) {
                locomotion.jump.arc_height = arc.max(0.);
            }
            note_unknown(
                jump,
                "locomotion.jump",
                &["enabled", "max_gap", "arc_height"],
                report,
            );
        }
    }
    if let Some(gravity) = number(map, "locomotion", "gravity", false, report) {
        locomotion.gravity = gravity.clamp(0., 4.);
    }
    if let Some(style) = string(map, "locomotion", "air_style", false, report) {
        match AirStyle::parse(&style) {
            Some(style) => locomotion.air_style = style,
            None => report.push(Diagnostic::error(
                MANIFEST_FILE,
                "locomotion.air_style",
                format!("Found `{style}`."),
                "Expected one of: straight, flap_arc, flutter, glide.",
            )),
        }
    }
    if let Some(min) = number(map, "locomotion", "min_surface_length", false, report) {
        locomotion.min_surface_length = min.max(0.);
    }
    if locomotion.class == LocomotionClass::Flier && locomotion.speed.air <= 0. {
        report.push(Diagnostic::warning(
            MANIFEST_FILE,
            "locomotion.speed.air",
            "A flier with no air speed will never get anywhere.",
            "Set `speed.air` to something like 220 pixels per second.",
        ));
        locomotion.speed.air = 220.;
    }
    note_unknown(
        map,
        "locomotion",
        &[
            "class",
            "surfaces",
            "speed",
            "jump",
            "gravity",
            "air_style",
            "min_surface_length",
        ],
        report,
    );
    Some(locomotion)
}

/// Shorthand groups keep the common cases to one word.
pub fn expand_surfaces(names: &[&str]) -> Vec<SurfaceKind> {
    use SurfaceKind::*;
    let mut out = Vec::new();
    for name in names {
        let group: &[SurfaceKind] = match *name {
            "window_any" => &[WindowTop, WindowBottom, WindowLeft, WindowRight],
            "window_vertical" => &[WindowLeft, WindowRight],
            "monitor_any" => &[MonitorTop, MonitorBottom, MonitorLeft, MonitorRight],
            "all" => &SurfaceKind::ALL,
            other => match SurfaceKind::parse(other) {
                Some(kind) => {
                    out.push(kind);
                    continue;
                }
                None => &[],
            },
        };
        out.extend_from_slice(group);
    }
    out.sort_by_key(|k| k.as_str());
    out.dedup();
    out
}

fn read_animations(root: &Map<String, Value>, report: &mut Report) -> BTreeMap<String, Animation> {
    let mut animations = BTreeMap::new();
    let Some(value) = root.get("animations") else {
        report.push(Diagnostic::error(
            MANIFEST_FILE,
            "animations",
            "The animations block is missing.",
            "Provide at least one sprite strip, e.g. `idle`.",
        ));
        return animations;
    };
    let Some(map) = expect_object(value, "animations", report) else {
        return animations;
    };
    for (name, clip) in map {
        let path = format!("animations.{name}");
        let Some(clip) = expect_object(clip, &path, report) else {
            continue;
        };
        let Some(file) = string(clip, &path, "file", true, report) else {
            continue;
        };
        let frames = match clip.get("frames") {
            None => {
                report.push(Diagnostic::error(
                    MANIFEST_FILE,
                    format!("{path}.frames"),
                    "The frame count is missing.",
                    "Set `frames` to how many frames the strip contains, e.g. 4.",
                ));
                continue;
            }
            Some(value) => match value.as_u64() {
                Some(n) if n > 0 => n as u32,
                _ => {
                    report.push(Diagnostic::error(
                        MANIFEST_FILE,
                        format!("{path}.frames"),
                        format!("Expected a positive integer, found {}.", describe(value)),
                        "Frame counts must be numbers, not quoted strings.",
                    ));
                    continue;
                }
            },
        };
        let fps = number(clip, &path, "fps", false, report).unwrap_or(8.);
        if fps <= 0. {
            report.push(Diagnostic::error(
                MANIFEST_FILE,
                format!("{path}.fps"),
                format!("Found {fps}."),
                "Frames per second must be greater than zero.",
            ));
            continue;
        }
        if fps > 30. && frames <= 2 {
            report.push(Diagnostic::warning(
                MANIFEST_FILE,
                format!("{path}.fps"),
                format!("{fps} frames per second across only {frames} frames."),
                "This will flicker. Slow it down or add frames.",
            ));
        }
        let mut offset = (0., 0.);
        if let Some(value) = clip.get("offset") {
            if let Some(o) = expect_object(value, &format!("{path}.offset"), report) {
                offset = (
                    number(o, &format!("{path}.offset"), "x", false, report).unwrap_or(0.),
                    number(o, &format!("{path}.offset"), "y", false, report).unwrap_or(0.),
                );
            }
        }
        note_unknown(
            clip,
            &path,
            &[
                "file", "frames", "fps", "loop", "pingpong", "sound", "offset",
            ],
            report,
        );
        animations.insert(
            name.clone(),
            Animation {
                file,
                frames,
                fps,
                looping: boolean(clip, &path, "loop", false, report).unwrap_or(true),
                pingpong: boolean(clip, &path, "pingpong", false, report).unwrap_or(false),
                sound: string(clip, &path, "sound", false, report),
                offset_x: offset.0,
                offset_y: offset.1,
            },
        );
    }
    if animations.is_empty() {
        report.push(Diagnostic::error(
            MANIFEST_FILE,
            "animations",
            "The pack has no animations.",
            "Provide at least one sprite strip, e.g. `idle`; every other clip falls back to it.",
        ));
    }
    // Idle and move are not required: each stands in for the other.
    for (clip, other) in [("idle", "move"), ("move", "idle")] {
        if !animations.contains_key(clip) && animations.contains_key(other) {
            report.push(Diagnostic::info(
                MANIFEST_FILE,
                format!("animations.{clip}"),
                format!("No `{clip}` animation, so this pet plays `{other}` instead."),
                format!("Add an `animations.{clip}` clip to give it its own look."),
            ));
        }
    }
    // Optional, but silently missing would leave an author wondering why their
    // pet looks the same whether it is working or waiting on them.
    for (clip, when) in ASSISTANT_CLIPS {
        if animations.contains_key(*clip) {
            continue;
        }
        let fallback = CLIP_FALLBACKS
            .iter()
            .find(|(name, _)| name == clip)
            .and_then(|(_, chain)| chain.iter().find(|c| animations.contains_key(**c)))
            .copied()
            .unwrap_or("idle");
        report.push(Diagnostic::info(
            MANIFEST_FILE,
            format!("animations.{clip}"),
            format!("No `{clip}` animation, so this pet plays `{fallback}` {when}."),
            format!("Add an `animations.{clip}` clip to give it its own look."),
        ));
    }
    animations
}

fn read_behavior(
    value: &Value,
    animations: &BTreeMap<String, Animation>,
    report: &mut Report,
) -> Behavior {
    let mut behavior = Behavior {
        initial: String::new(),
        idle_wander_chance: 0.3,
        states: BTreeMap::new(),
    };
    let Some(map) = expect_object(value, "behavior", report) else {
        return default_behavior(LocomotionClass::Flier);
    };
    if let Some(chance) = number(map, "behavior", "idle_wander_chance", false, report) {
        behavior.idle_wander_chance = chance.clamp(0., 1.);
    }
    let states = map
        .get("states")
        .and_then(|s| expect_object(s, "behavior.states", report));
    let Some(states) = states else {
        report.push(Diagnostic::error(
            MANIFEST_FILE,
            "behavior.states",
            "The behavior block has no states.",
            "Add at least one state, or omit `behavior` for the class default.",
        ));
        return default_behavior(LocomotionClass::Flier);
    };
    for (name, state) in states {
        let path = format!("behavior.states.{name}");
        let Some(state) = expect_object(state, &path, report) else {
            continue;
        };
        let clip = string(state, &path, "clip", false, report).unwrap_or_else(|| "idle".into());
        if !animations.contains_key(&clip)
            && !CLIP_FALLBACKS.iter().any(|(n, _)| *n == clip)
            && clip != "idle"
            && clip != "move"
        {
            report.push(Diagnostic::warning(
                MANIFEST_FILE,
                format!("{path}.clip"),
                format!("`{clip}` is not one of this pack's animations."),
                "The engine will fall back to `idle`. Check the spelling.",
            ));
        }
        let duration = read_duration(state, &path, report);
        let action = match string(state, &path, "action", false, report) {
            None => None,
            Some(name) => match Action::parse(&name) {
                Some(action) => Some(action),
                None => {
                    report.push(Diagnostic::error(
                        MANIFEST_FILE,
                        format!("{path}.action"),
                        format!("Unknown action `{name}`."),
                        format!(
                            "Expected one of: {}.",
                            Action::ALL
                                .iter()
                                .map(|a| a.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    ));
                    None
                }
            },
        };
        let mut transitions = Vec::new();
        if let Some(list) = state.get("transitions") {
            if let Some(items) = expect_array(list, &format!("{path}.transitions"), report) {
                for (i, item) in items.iter().enumerate() {
                    let path = format!("{path}.transitions[{i}]");
                    let Some(item) = expect_object(item, &path, report) else {
                        continue;
                    };
                    let Some(to) = string(item, &path, "to", true, report) else {
                        continue;
                    };
                    let when = match string(item, &path, "when", false, report) {
                        None => None,
                        Some(text) => match Condition::parse(&text) {
                            Ok(condition) => Some(condition),
                            Err(problem) => {
                                report.push(Diagnostic::error(
                                    MANIFEST_FILE,
                                    format!("{path}.when"),
                                    problem,
                                    "Conditions look like `idle_seconds > 300`, joined by && or ||.",
                                ));
                                continue;
                            }
                        },
                    };
                    note_unknown(item, &path, &["to", "weight", "when"], report);
                    transitions.push(Transition {
                        to,
                        weight: number(item, &path, "weight", false, report)
                            .unwrap_or(1.)
                            .max(0.),
                        when,
                    });
                }
            }
        }
        note_unknown(
            state,
            &path,
            &["clip", "duration", "action", "sound", "transitions"],
            report,
        );
        behavior.states.insert(
            name.clone(),
            State {
                clip,
                duration,
                action,
                sound: string(state, &path, "sound", false, report),
                transitions,
            },
        );
    }
    behavior.initial = string(map, "behavior", "initial", false, report)
        .or_else(|| behavior.states.keys().next().cloned())
        .unwrap_or_else(|| "idle".into());
    if !behavior.states.contains_key(&behavior.initial) {
        report.push(Diagnostic::error(
            MANIFEST_FILE,
            "behavior.initial",
            format!("`{}` is not one of the declared states.", behavior.initial),
            format!(
                "Expected one of: {}.",
                behavior
                    .states
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }
    let known: Vec<String> = behavior.states.keys().cloned().collect();
    for (name, state) in &behavior.states {
        for transition in &state.transitions {
            if !known.contains(&transition.to) {
                report.push(Diagnostic::error(
                    MANIFEST_FILE,
                    format!("behavior.states.{name}.transitions"),
                    format!("`{}` is not a declared state.", transition.to),
                    format!("Expected one of: {}.", known.join(", ")),
                ));
            }
        }
    }
    note_unknown(
        map,
        "behavior",
        &["initial", "idle_wander_chance", "states"],
        report,
    );
    if behavior.states.is_empty() {
        return default_behavior(LocomotionClass::Flier);
    }
    behavior
}

fn read_duration(state: &Map<String, Value>, path: &str, report: &mut Report) -> (f64, f64) {
    match state.get("duration") {
        None => (4., 10.),
        Some(Value::Number(n)) => {
            let v = n.as_f64().unwrap_or(4.).max(0.);
            (v, v)
        }
        Some(Value::Array(items)) if items.len() == 2 => {
            let read = |i: usize| items[i].as_f64().unwrap_or(f64::NAN);
            let (a, b) = (read(0), read(1));
            if a.is_nan() || b.is_nan() || a < 0. || b < a {
                report.push(Diagnostic::error(
                    MANIFEST_FILE,
                    format!("{path}.duration"),
                    format!("Found {:?}.", items),
                    "A range is `[min, max]` in seconds, with min ≤ max.",
                ));
                (4., 10.)
            } else {
                (a, b)
            }
        }
        Some(other) => {
            report.push(Diagnostic::error(
                MANIFEST_FILE,
                format!("{path}.duration"),
                format!("Found {}.", describe(other)),
                "Use seconds as a number, or `[min, max]` for a random range.",
            ));
            (4., 10.)
        }
    }
}

fn read_reactions(
    root: &Map<String, Value>,
    behavior: &Behavior,
    report: &mut Report,
) -> Vec<Reaction> {
    let mut reactions = Vec::new();
    let Some(value) = root.get("reactions") else {
        return reactions;
    };
    let Some(items) = expect_array(value, "reactions", report) else {
        return reactions;
    };
    for (i, item) in items.iter().enumerate() {
        let path = format!("reactions[{i}]");
        let Some(item) = expect_object(item, &path, report) else {
            continue;
        };
        let Some(on) = string(item, &path, "on", true, report) else {
            continue;
        };
        let Some(event) = Event::parse(&on) else {
            report.push(Diagnostic::error(
                MANIFEST_FILE,
                format!("{path}.on"),
                format!("Unknown event `{on}`."),
                format!(
                    "Expected one of: {}.",
                    Event::ALL
                        .iter()
                        .map(|e| e.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
            continue;
        };
        let Some(goto) = string(item, &path, "goto", true, report) else {
            continue;
        };
        if !behavior.states.contains_key(&goto) {
            report.push(Diagnostic::error(
                MANIFEST_FILE,
                format!("{path}.goto"),
                format!("`{goto}` is not a declared state."),
                format!(
                    "Expected one of: {}.",
                    behavior
                        .states
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
            continue;
        }
        note_unknown(item, &path, &["on", "goto", "priority", "cooldown"], report);
        reactions.push(Reaction {
            on: event,
            goto,
            priority: number(item, &path, "priority", false, report)
                .map(|p| p as i32)
                .unwrap_or_else(|| event.default_priority()),
            cooldown: number(item, &path, "cooldown", false, report)
                .unwrap_or(0.)
                .max(0.),
        });
    }
    reactions.sort_by_key(|r| std::cmp::Reverse(r.priority));
    reactions
}

fn read_personality(root: &Map<String, Value>, report: &mut Report) -> Personality {
    let mut personality = Personality::default();
    let Some(value) = root.get("personality") else {
        return personality;
    };
    let Some(map) = expect_object(value, "personality", report) else {
        return personality;
    };
    for (key, target) in [
        ("activity", &mut personality.activity),
        ("boldness", &mut personality.boldness),
        ("sociability", &mut personality.sociability),
        ("sleepiness", &mut personality.sleepiness),
        ("curiosity", &mut personality.curiosity),
    ] {
        if let Some(v) = number(map, "personality", key, false, report) {
            if !(0. ..=1.).contains(&v) {
                report.push(Diagnostic::warning(
                    MANIFEST_FILE,
                    format!("personality.{key}"),
                    format!("Found {v}."),
                    "Personality scalars run from 0 to 1; the value has been clamped.",
                ));
            }
            *target = v.clamp(0., 1.);
        }
    }
    note_unknown(
        map,
        "personality",
        &[
            "activity",
            "boldness",
            "sociability",
            "sleepiness",
            "curiosity",
        ],
        report,
    );
    personality
}

/// The per-class default profile used when a pack omits `behavior` entirely.
pub fn default_behavior(class: LocomotionClass) -> Behavior {
    let mut states = BTreeMap::new();
    let settle = if class == LocomotionClass::Flier {
        "perch"
    } else {
        "rest"
    };
    states.insert(
        settle.to_string(),
        State {
            clip: "idle".into(),
            duration: (6., 18.),
            action: None,
            sound: None,
            transitions: vec![
                Transition {
                    to: "relocate".into(),
                    weight: 3.,
                    when: None,
                },
                Transition {
                    to: "doze".into(),
                    weight: 5.,
                    when: Condition::parse("idle_seconds > 240").ok(),
                },
                Transition {
                    to: "visit".into(),
                    weight: 1.,
                    when: Condition::parse("pet_distance < 1500").ok(),
                },
                Transition {
                    to: "doze".into(),
                    weight: 4.,
                    when: Condition::parse("pet_distance < 120 && energy < 0.4").ok(),
                },
            ],
        },
    );
    states.insert(
        "relocate".to_string(),
        State {
            clip: "move".into(),
            duration: (0., 0.),
            action: Some(Action::MoveToRandomSurface),
            sound: None,
            transitions: vec![Transition {
                to: settle.to_string(),
                weight: 1.,
                when: None,
            }],
        },
    );
    states.insert(
        "visit".to_string(),
        State {
            clip: "move".into(),
            duration: (0., 0.),
            action: Some(Action::MoveTowardPet),
            sound: None,
            transitions: vec![Transition {
                to: settle.to_string(),
                weight: 1.,
                when: None,
            }],
        },
    );
    states.insert(
        "doze".to_string(),
        State {
            clip: "sleep".into(),
            duration: (30., 120.),
            action: None,
            sound: None,
            transitions: vec![Transition {
                to: settle.to_string(),
                weight: 1.,
                when: Condition::parse("idle_seconds < 10").ok(),
            }],
        },
    );
    states.insert(
        "greet".to_string(),
        State {
            clip: "happy".into(),
            duration: (2., 3.),
            action: None,
            sound: None,
            transitions: vec![Transition {
                to: settle.to_string(),
                weight: 1.,
                when: None,
            }],
        },
    );
    states.insert(
        "startled".to_string(),
        State {
            clip: "alert".into(),
            duration: (1., 2.),
            action: Some(Action::MoveToNearestSurface),
            sound: None,
            transitions: vec![Transition {
                to: settle.to_string(),
                weight: 1.,
                when: None,
            }],
        },
    );
    states.insert(
        "held".to_string(),
        State {
            clip: "grabbed".into(),
            duration: (0.5, 0.5),
            action: None,
            sound: None,
            transitions: vec![Transition {
                to: settle.to_string(),
                weight: 1.,
                when: None,
            }],
        },
    );
    // The throw and petting reactions every pet gets; a pack without the
    // clips plays alert/happy/idle instead.
    for (name, clip, duration) in [("dizzy", "dizzy", (2., 3.)), ("enjoy", "petted", (2., 4.))] {
        states.insert(
            name.to_string(),
            State {
                clip: clip.into(),
                duration,
                action: None,
                sound: None,
                transitions: vec![Transition {
                    to: settle.to_string(),
                    weight: 1.,
                    when: None,
                }],
            },
        );
    }
    Behavior {
        initial: settle.to_string(),
        idle_wander_chance: 0.3,
        states,
    }
}

/// The reactions every pet gets when the pack does not declare its own.
pub fn default_reactions(behavior: &Behavior) -> Vec<Reaction> {
    [
        (Event::SurfaceLost, "startled", 100, 0.),
        (Event::Grabbed, "held", 90, 0.),
        (Event::UserReturned, "greet", 60, 60.),
        (Event::Clicked, "greet", 90, 2.),
        (Event::UserIdleLong, "doze", 30, 0.),
        (Event::Thrown, "dizzy", 90, 0.),
        (Event::LandedHard, "dizzy", 90, 0.),
        (Event::Petted, "enjoy", 60, 3.),
        (Event::PetNearby, "greet", 50, 120.),
        (Event::NewDay, "greet", 50, 600.),
    ]
    .into_iter()
    .filter(|(_, goto, _, _)| behavior.states.contains_key(*goto))
    .map(|(on, goto, priority, cooldown)| Reaction {
        on,
        goto: goto.to_string(),
        priority,
        cooldown,
    })
    .collect()
}

// ---------------------------------------------------------------- readers

fn describe(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(b) => format!("{b} (true/false)"),
        Value::Number(n) => format!("{n} (number)"),
        Value::String(s) => format!("{s:?} (string)"),
        Value::Array(_) => "a list".into(),
        Value::Object(_) => "an object".into(),
    }
}

fn expect_object<'a>(
    value: &'a Value,
    path: &str,
    report: &mut Report,
) -> Option<&'a Map<String, Value>> {
    match value.as_object() {
        Some(map) => Some(map),
        None => {
            report.push(Diagnostic::error(
                MANIFEST_FILE,
                path,
                format!("Expected an object, found {}.", describe(value)),
                "Wrap the settings in `{ ... }`.",
            ));
            None
        }
    }
}

fn expect_array<'a>(value: &'a Value, path: &str, report: &mut Report) -> Option<&'a Vec<Value>> {
    match value.as_array() {
        Some(items) => Some(items),
        None => {
            report.push(Diagnostic::error(
                MANIFEST_FILE,
                path,
                format!("Expected a list, found {}.", describe(value)),
                "Wrap the entries in `[ ... ]`.",
            ));
            None
        }
    }
}

fn missing(path: &str, key: &str, expected: &str, report: &mut Report) {
    let full = if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    };
    report.push(Diagnostic::error(
        MANIFEST_FILE,
        full,
        format!("The required field `{key}` is missing."),
        expected.to_string(),
    ));
}

fn field_path(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

fn string(
    map: &Map<String, Value>,
    path: &str,
    key: &str,
    required: bool,
    report: &mut Report,
) -> Option<String> {
    match map.get(key) {
        None => {
            if required {
                missing(path, key, "Expected a piece of text in quotes.", report);
            }
            None
        }
        Some(Value::String(text)) => Some(text.clone()),
        Some(other) => {
            report.push(Diagnostic::error(
                MANIFEST_FILE,
                field_path(path, key),
                format!("Expected text, found {}.", describe(other)),
                "Put the value in double quotes.",
            ));
            None
        }
    }
}

fn strings(map: &Map<String, Value>, path: &str, key: &str, report: &mut Report) -> Vec<String> {
    match map.get(key) {
        None => Vec::new(),
        Some(value) => expect_array(value, &field_path(path, key), report)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|i| i.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
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
        None => {
            if required {
                missing(path, key, "Expected a number.", report);
            }
            None
        }
        Some(Value::Number(n)) => n.as_f64(),
        Some(other) => {
            report.push(Diagnostic::error(
                MANIFEST_FILE,
                field_path(path, key),
                format!("Expected a number, found {}.", describe(other)),
                "Numbers are written without quotes, e.g. 48.",
            ));
            None
        }
    }
}

fn integer(
    map: &Map<String, Value>,
    path: &str,
    key: &str,
    required: bool,
    report: &mut Report,
) -> Option<u64> {
    match map.get(key) {
        None => {
            if required {
                missing(path, key, "Expected a whole number.", report);
            }
            None
        }
        Some(value) => match value.as_u64() {
            Some(n) => Some(n),
            None => {
                report.push(Diagnostic::error(
                    MANIFEST_FILE,
                    field_path(path, key),
                    format!("Expected a whole number, found {}.", describe(value)),
                    "Whole numbers are written without quotes or decimals, e.g. 1.",
                ));
                None
            }
        },
    }
}

fn boolean(
    map: &Map<String, Value>,
    path: &str,
    key: &str,
    required: bool,
    report: &mut Report,
) -> Option<bool> {
    match map.get(key) {
        None => {
            if required {
                missing(path, key, "Expected true or false.", report);
            }
            None
        }
        Some(Value::Bool(b)) => Some(*b),
        Some(other) => {
            report.push(Diagnostic::error(
                MANIFEST_FILE,
                field_path(path, key),
                format!("Expected true or false, found {}.", describe(other)),
                "Write true or false without quotes.",
            ));
            None
        }
    }
}

/// Unknown fields are ignored so newer packs degrade rather than fail, but the
/// author is told, because a typo looks exactly like a field from the future.
fn note_unknown(map: &Map<String, Value>, path: &str, known: &[&str], report: &mut Report) {
    for key in map.keys() {
        if !known.contains(&key.as_str()) {
            report.push(Diagnostic::new(
                Tier::Info,
                MANIFEST_FILE,
                field_path(path, key),
                format!("Unknown field `{key}` was ignored."),
                "Check the spelling, or ignore this if the pack targets a newer Ledgeling.",
            ));
        }
    }
}
