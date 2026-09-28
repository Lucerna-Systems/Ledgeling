//! Desktop apps started from coding mode's terminals, such as a Tauri or
//! Electron app under its dev command: which of a terminal's processes own a
//! window, and bringing one forward. Only process ids are used; no window
//! title is ever read.
use std::collections::HashSet;

/// The processes in `candidates` that are regular windowed apps.
pub fn windowed(candidates: &HashSet<u32>) -> HashSet<u32> {
    imp::windowed(candidates)
}

/// Bring the app forward, restoring it if it was minimized or hidden.
pub fn activate(pid: u32) -> Result<(), String> {
    imp::activate(pid)
}

#[cfg(target_os = "macos")]
mod imp {
    use std::collections::HashSet;

    unsafe extern "C" {
        fn perch_app_is_gui(pid: i32) -> i32;
        fn perch_app_activate(pid: i32) -> i32;
    }

    pub fn windowed(candidates: &HashSet<u32>) -> HashSet<u32> {
        candidates
            .iter()
            .copied()
            .filter(|pid| unsafe { perch_app_is_gui(*pid as i32) } == 1)
            .collect()
    }

    pub fn activate(pid: u32) -> Result<(), String> {
        if unsafe { perch_app_activate(pid as i32) } == 1 {
            Ok(())
        } else {
            Err("That app has closed.".into())
        }
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use std::collections::HashSet;

    type Handle = isize;
    type Bool = i32;
    const GW_OWNER: u32 = 4;
    const SW_RESTORE: i32 = 9;

    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(callback: extern "system" fn(Handle, isize) -> Bool, param: isize) -> Bool;
        fn IsWindowVisible(window: Handle) -> Bool;
        fn IsIconic(window: Handle) -> Bool;
        fn GetWindow(window: Handle, command: u32) -> Handle;
        fn GetWindowThreadProcessId(window: Handle, pid: *mut u32) -> u32;
        fn ShowWindow(window: Handle, command: i32) -> Bool;
        fn SetForegroundWindow(window: Handle) -> Bool;
    }

    extern "system" fn collect(window: Handle, param: isize) -> Bool {
        let found = unsafe { &mut *(param as *mut Vec<(Handle, u32)>) };
        unsafe {
            if IsWindowVisible(window) != 0 && GetWindow(window, GW_OWNER) == 0 {
                let mut pid = 0u32;
                GetWindowThreadProcessId(window, &mut pid);
                found.push((window, pid));
            }
        }
        1
    }

    /// Visible, unowned top-level windows: an app's main windows.
    fn top_level() -> Vec<(Handle, u32)> {
        let mut found: Vec<(Handle, u32)> = Vec::new();
        unsafe {
            EnumWindows(collect, &mut found as *mut _ as isize);
        }
        found
    }

    pub fn windowed(candidates: &HashSet<u32>) -> HashSet<u32> {
        top_level()
            .into_iter()
            .map(|(_, pid)| pid)
            .filter(|pid| candidates.contains(pid))
            .collect()
    }

    pub fn activate(pid: u32) -> Result<(), String> {
        let (window, _) = top_level()
            .into_iter()
            .find(|(_, p)| *p == pid)
            .ok_or("That app has closed.")?;
        unsafe {
            if IsIconic(window) != 0 {
                ShowWindow(window, SW_RESTORE);
            }
            SetForegroundWindow(window);
        }
        Ok(())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    use std::collections::HashSet;

    pub fn windowed(_candidates: &HashSet<u32>) -> HashSet<u32> {
        HashSet::new()
    }

    pub fn activate(_pid: u32) -> Result<(), String> {
        Err("Bringing an app forward is not supported on this system yet.".into())
    }
}
