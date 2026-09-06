use crate::{pet::Pet, platform::Display};
use serde::Serialize;
use tauri::{Emitter, LogicalPosition, WebviewWindow};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    pub clip: &'static str,
    pub frame_index: u32,
    pub flip_x: bool,
    pub x: f64,
    pub y: f64,
    pub greeting: bool,
    pub paused: bool,
}
pub trait PetSurface: Send {
    fn render(&mut self, pet: &Pet, display: Display) -> tauri::Result<()>;
}
pub struct NativeSurface {
    pub window: WebviewWindow,
    pub overlay: bool,
    last: Option<Frame>,
    last_position: Option<(i32, i32)>,
}
impl NativeSurface {
    pub fn new(window: WebviewWindow, overlay: bool) -> Self {
        Self {
            window,
            overlay,
            last: None,
            last_position: None,
        }
    }
}
impl PetSurface for NativeSurface {
    fn render(&mut self, pet: &Pet, display: Display) -> tauri::Result<()> {
        let x = (pet.position.x / display.scale - 64.).round();
        let y = (pet.position.y / display.scale - 112.).round();
        if !self.overlay && self.last_position != Some((x as i32, y as i32)) {
            self.window.set_position(LogicalPosition::new(x, y))?;
            self.last_position = Some((x as i32, y as i32));
        }
        let frame = Frame {
            clip: pet.clip(),
            frame_index: pet.frame_index(),
            flip_x: pet.flip_x,
            x: if self.overlay {
                x - display.bounds.x / display.scale
            } else {
                0.
            },
            y: if self.overlay {
                y - display.bounds.y / display.scale
            } else {
                0.
            },
            greeting: pet.activity == crate::pet::Activity::Greeting,
            paused: pet.paused,
        };
        if self.last.as_ref() != Some(&frame) {
            self.window.emit("pet:frame", &frame)?;
            self.last = Some(frame);
        }
        Ok(())
    }
}

pub fn hit_test(pet: &Pet, display: Display, cursor: crate::world::Point) -> bool {
    let x = ((cursor.x - pet.position.x) / display.scale / 4. + 16.).floor() as i32;
    let y = ((cursor.y - pet.position.y) / display.scale / 4. + 28.).floor() as i32;
    if !(0..32).contains(&x) || !(0..32).contains(&y) {
        return false;
    }
    let x = if pet.flip_x { 31 - x } else { x };
    let masks: &[[u32; 32]; 8] = &include!("alpha_masks.in");
    let clip = if pet.clip() == "move" { 4 } else { 0 };
    masks[clip + pet.frame_index() as usize][y as usize] & (1_u32 << x) != 0
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{Point, Rect};
    #[test]
    fn transparent_corners_click_through_but_body_is_solid() {
        let work = Rect {
            x: 0.,
            y: 0.,
            width: 1000.,
            height: 800.,
        };
        let p = Pet::new(work);
        let d = Display {
            bounds: work,
            work,
            scale: 1.,
        };
        assert!(!hit_test(
            &p,
            d,
            Point {
                x: p.position.x - 63.,
                y: p.position.y - 111.
            }
        ));
        assert!(hit_test(
            &p,
            d,
            Point {
                x: p.position.x,
                y: p.position.y - 40.
            }
        ));
    }
}
