//! "Make room at the top": a window pressed against the top of the screen
//! hides whatever stands on its top edge. With the user's say-so, Ledgeling
//! moves that window's top edge down just far enough, only for windows that
//! have a pet or a biome on top, and puts it back afterwards unless the user
//! has moved the window since. Where windows cannot be moved, pets keep off
//! such edges instead (see `cramped`).
use crate::{
    platform::{Platform, ResizeAccess},
    world::{Rect, WindowInfo},
};
use std::collections::{HashMap, HashSet};

/// Extra space above what stands on the edge, physical pixels per scale.
const MARGIN: f64 = 8.;
/// A window is never squeezed below this height, logical pixels.
const MIN_HEIGHT: f64 = 200.;
/// Seconds a window must have nothing on top before it is put back.
const RESTORE_AFTER: f64 = 10.;
/// A window scan may still show the old frame this long after a move.
const SETTLE: f64 = 1.;
/// How far a window may be from where it was put before that counts as the
/// user having moved it.
const TOLERANCE: f64 = 3.;
/// Seconds before a window that refused to move is tried again: permission
/// may have been granted since, or the app may have been busy.
const RETRY_AFTER: f64 = 30.;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Record {
    original_top: f64,
    our_top: f64,
    moved_at: f64,
    idle_since: Option<f64>,
}

#[derive(Default)]
pub struct Room {
    records: HashMap<u64, Record>,
    /// Windows the user moved back after Ledgeling moved them. Never moved
    /// again; pets keep off their tops.
    declined: HashSet<u64>,
    /// Windows that refused to move, and when; tried again later.
    failed: HashMap<u64, f64>,
    /// Why the last move failed, in plain words, for Settings.
    pub last_error: Option<String>,
}

/// One window that has something on its top edge, and how tall that is.
#[derive(Clone, Copy, Debug)]
pub struct Need {
    pub window: u64,
    pub height: f64,
}

impl Room {
    /// Called after each window scan while the feature is on.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        clock: f64,
        windows: &[WindowInfo],
        work_top: f64,
        scale: f64,
        needs: &[Need],
        access: ResizeAccess,
        mouse_down: bool,
        platform: &dyn Platform,
    ) {
        self.declined
            .retain(|id| windows.iter().any(|w| w.id == *id));
        self.failed
            .retain(|id, at| windows.iter().any(|w| w.id == *id) && clock - *at < RETRY_AFTER);
        // Forget windows that closed. One the user has moved since is theirs:
        // it is never put back, and never moved again either.
        let mut declined = Vec::new();
        self.records.retain(|id, record| {
            let Some(w) = windows.iter().find(|w| w.id == *id) else {
                return false;
            };
            let kept =
                clock - record.moved_at < SETTLE || (w.rect.y - record.our_top).abs() <= TOLERANCE;
            if !kept {
                declined.push(*id);
            }
            kept
        });
        self.declined.extend(declined);
        if access != ResizeAccess::Granted || mouse_down {
            return;
        }
        for window in windows {
            let need = needs.iter().find(|n| n.window == window.id);
            if let Some(record) = self.records.get_mut(&window.id) {
                record.idle_since = match (need, record.idle_since) {
                    (Some(_), _) => None,
                    (None, None) => Some(clock),
                    (None, since) => since,
                };
                if record
                    .idle_since
                    .is_some_and(|since| clock - since >= RESTORE_AFTER)
                {
                    let original = record.original_top;
                    self.records.remove(&window.id);
                    let _ = platform.set_window_frame(window, with_top(window.rect, original));
                }
                continue;
            }
            let Some(need) = need else {
                continue;
            };
            if self.refused(window.id) {
                continue;
            }
            let wanted = work_top + need.height + MARGIN * scale;
            if window.rect.y >= wanted - 1. {
                continue;
            }
            let target = with_top(window.rect, wanted);
            if target.height < MIN_HEIGHT * scale {
                continue;
            }
            match platform.set_window_frame(window, target) {
                Ok(()) => {
                    self.last_error = None;
                    self.records.insert(
                        window.id,
                        Record {
                            original_top: window.rect.y,
                            our_top: wanted,
                            moved_at: clock,
                            idle_since: None,
                        },
                    );
                }
                Err(error) => {
                    self.failed.insert(window.id, clock);
                    self.last_error = Some(error);
                }
            }
        }
    }

    /// Put every window back that is still where Ledgeling left it: the
    /// feature was switched off, or Ledgeling is quitting.
    pub fn restore_all(&mut self, windows: &[WindowInfo], platform: &dyn Platform) {
        for (id, record) in self.records.drain() {
            if let Some(window) = windows.iter().find(|w| w.id == id) {
                if (window.rect.y - record.our_top).abs() <= TOLERANCE {
                    let _ = platform
                        .set_window_frame(window, with_top(window.rect, record.original_top));
                }
            }
        }
        self.declined.clear();
        self.failed.clear();
        self.last_error = None;
    }

    pub fn is_holding(&self, window: u64) -> bool {
        self.records.contains_key(&window)
    }

    /// Declined by the user, or refused recently: pets keep off it for now.
    pub fn refused(&self, window: u64) -> bool {
        self.declined.contains(&window) || self.failed.contains_key(&window)
    }
}

/// The same window with its top edge at `top` and its bottom where it was.
fn with_top(rect: Rect, top: f64) -> Rect {
    Rect::new(rect.x, top, rect.width, rect.bottom() - top)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::mock::MockPlatform;

    fn window(top: f64) -> WindowInfo {
        WindowInfo::new(1, Rect::new(100., top, 800., 700.))
    }

    fn platform(top: f64) -> MockPlatform {
        let platform = MockPlatform::new(Vec::new(), vec![vec![window(top)]]);
        *platform.access.lock().unwrap() = ResizeAccess::Granted;
        platform
    }

    fn scan(platform: &MockPlatform) -> Vec<WindowInfo> {
        platform.enumerate_windows().unwrap()
    }

    const NEED: [Need; 1] = [Need {
        window: 1,
        height: 64.,
    }];

    #[test]
    fn a_window_at_the_top_with_a_pet_on_it_is_moved_down_just_enough() {
        let platform = platform(25.);
        let mut room = Room::default();
        room.update(
            0.,
            &scan(&platform),
            25.,
            1.,
            &NEED,
            ResizeAccess::Granted,
            false,
            &platform,
        );
        let moves = platform.moves.lock().unwrap().clone();
        assert_eq!(moves, vec![(1, Rect::new(100., 97., 800., 628.))]);
        assert!(room.is_holding(1));
        // The next scans see it in place: nothing more happens.
        room.update(
            0.5,
            &scan(&platform),
            25.,
            1.,
            &NEED,
            ResizeAccess::Granted,
            false,
            &platform,
        );
        room.update(
            3.,
            &scan(&platform),
            25.,
            1.,
            &NEED,
            ResizeAccess::Granted,
            false,
            &platform,
        );
        assert_eq!(platform.moves.lock().unwrap().len(), 1);
    }

    #[test]
    fn nothing_moves_while_the_mouse_is_down_or_without_permission() {
        let platform = platform(25.);
        let mut room = Room::default();
        room.update(
            0.,
            &scan(&platform),
            25.,
            1.,
            &NEED,
            ResizeAccess::Granted,
            true,
            &platform,
        );
        room.update(
            0.,
            &scan(&platform),
            25.,
            1.,
            &NEED,
            ResizeAccess::NeedsPermission,
            false,
            &platform,
        );
        assert!(platform.moves.lock().unwrap().is_empty());
    }

    #[test]
    fn a_window_is_put_back_once_nothing_has_stood_on_it_for_a_while() {
        let platform = platform(25.);
        let mut room = Room::default();
        room.update(
            0.,
            &scan(&platform),
            25.,
            1.,
            &NEED,
            ResizeAccess::Granted,
            false,
            &platform,
        );
        room.update(
            5.,
            &scan(&platform),
            25.,
            1.,
            &[],
            ResizeAccess::Granted,
            false,
            &platform,
        );
        assert_eq!(platform.moves.lock().unwrap().len(), 1, "not yet");
        room.update(
            15.5,
            &scan(&platform),
            25.,
            1.,
            &[],
            ResizeAccess::Granted,
            false,
            &platform,
        );
        let moves = platform.moves.lock().unwrap().clone();
        assert_eq!(moves.last(), Some(&(1, Rect::new(100., 25., 800., 700.))));
        assert!(!room.is_holding(1));
    }

    #[test]
    fn a_window_the_user_moved_is_never_put_back() {
        let platform = platform(25.);
        let mut room = Room::default();
        room.update(
            0.,
            &scan(&platform),
            25.,
            1.,
            &NEED,
            ResizeAccess::Granted,
            false,
            &platform,
        );
        platform.frames.lock().unwrap()[0][0].rect = Rect::new(300., 300., 600., 400.);
        room.update(
            2.,
            &scan(&platform),
            25.,
            1.,
            &NEED,
            ResizeAccess::Granted,
            false,
            &platform,
        );
        assert!(!room.is_holding(1));
        assert!(room.refused(1), "and it is not fought over");
        room.restore_all(&scan(&platform), &platform);
        assert_eq!(
            platform.moves.lock().unwrap().len(),
            1,
            "only the first move"
        );
    }

    #[test]
    fn switching_off_puts_windows_back_and_a_refusal_is_remembered() {
        let platform = platform(25.);
        let mut room = Room::default();
        room.update(
            0.,
            &scan(&platform),
            25.,
            1.,
            &NEED,
            ResizeAccess::Granted,
            false,
            &platform,
        );
        room.restore_all(&scan(&platform), &platform);
        assert_eq!(platform.moves.lock().unwrap().last().unwrap().1.y, 25.);

        let stubborn = self::platform(25.);
        stubborn.stubborn.lock().unwrap().push(1);
        let mut room = Room::default();
        room.update(
            0.,
            &scan(&stubborn),
            25.,
            1.,
            &NEED,
            ResizeAccess::Granted,
            false,
            &stubborn,
        );
        assert!(room.refused(1));
        assert_eq!(room.last_error.as_deref(), Some("That window refused."));
        // Half a minute later it is tried again, and this time it moves.
        stubborn.stubborn.lock().unwrap().clear();
        let windows = scan(&stubborn);
        room.update(
            31.,
            &windows,
            25.,
            1.,
            &NEED,
            ResizeAccess::Granted,
            false,
            &stubborn,
        );
        assert!(room.is_holding(1));
        assert!(room.last_error.is_none());
    }

    #[test]
    fn a_short_window_is_not_squeezed() {
        let platform = MockPlatform::new(
            Vec::new(),
            vec![vec![WindowInfo::new(1, Rect::new(0., 25., 400., 240.))]],
        );
        *platform.access.lock().unwrap() = ResizeAccess::Granted;
        let mut room = Room::default();
        room.update(
            0.,
            &scan(&platform),
            25.,
            1.,
            &NEED,
            ResizeAccess::Granted,
            false,
            &platform,
        );
        assert!(platform.moves.lock().unwrap().is_empty());
    }
}
