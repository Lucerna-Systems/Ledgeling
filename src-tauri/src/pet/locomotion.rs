//! Per-class movement flavour: how fast a pet moves on a given surface, which
//! way its sprite points, and what shape its flight takes.
use crate::{
    packs::{manifest::AirStyle, Manifest, SurfaceOrientation},
    world::{Orientation, Point, Surface, SurfaceKind},
};

/// Surface speed in logical pixels per second. Vertical edges use `climb`,
/// falling back to the surface speed so a two-line pack still moves.
pub fn speed(manifest: &Manifest, surface: &Surface) -> f64 {
    let speeds = manifest.locomotion.speed;
    let base = match surface.orientation() {
        Orientation::Vertical if speeds.climb > 0. => speeds.climb,
        _ => speeds.surface,
    };
    if base > 0. {
        base
    } else {
        60.
    }
}

/// Degrees to rotate the sprite so it sits against the surface it is on.
/// `upright` packs never rotate — a bird perched on a window side would look
/// wrong lying on its face.
pub fn sprite_rotation(manifest: &Manifest, surface: &Surface) -> f64 {
    if manifest.sprite.surface_orientation == SurfaceOrientation::Upright {
        return 0.;
    }
    match surface.kind {
        SurfaceKind::WindowTop | SurfaceKind::TaskbarTop | SurfaceKind::MonitorBottom => 0.,
        SurfaceKind::WindowBottom | SurfaceKind::MonitorTop => 180.,
        // CSS rotation is clockwise, so the sprite's head turns to face the
        // surface normal: left for a window's left side, right for its right.
        SurfaceKind::WindowLeft | SurfaceKind::MonitorRight => -90.,
        SurfaceKind::WindowRight | SurfaceKind::MonitorLeft => 90.,
    }
}

/// Cosmetic deviation from a straight line during flight.
pub fn air_offset(style: AirStyle, t: f64, clock: f64, scale: f64) -> Point {
    let envelope = (std::f64::consts::PI * t).sin();
    match style {
        AirStyle::Straight => Point::new(0., 0.),
        AirStyle::FlapArc => Point::new(
            0.,
            -(envelope * 70. + (clock * 9.).sin() * 6. * envelope) * scale,
        ),
        AirStyle::Flutter => Point::new(
            (clock * 7.3).sin() * 26. * envelope * scale,
            ((clock * 5.1).cos() * 22. - envelope * 18.) * envelope * scale,
        ),
        AirStyle::Glide => Point::new(0., -(envelope * 26.) * scale),
    }
}
