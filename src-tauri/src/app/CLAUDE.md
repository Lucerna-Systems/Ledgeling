# App Submodule (`src-tauri/src/app/`)

## Purpose & Scope
The `app` module is the Tauri coordinator and platform adapter. It connects OS desktop windows, tray menus, display detection, and Tauri IPC commands to the headless [`Engine`](../engine.rs).

## Modular Structure
- `screens.rs`: Monitor enumeration, DPI scaling, and target pet display resolution.
- `menus.rs`: Native tray icon, application menu hierarchy, and menu click dispatching.
- `loop_runner.rs`: 60Hz/30Hz desktop rendering loop, debug overlays, and pack folder watching.
- `commands.rs`: Thin Tauri command wrappers invoking headless engine methods.
- `mod.rs`: Application builder, Tauri lifecycle setup, and plugin initialization.

## Architecture & Constraints
- **Keep app thin**: Business logic and pet simulation must remain in `engine.rs` and domain submodules.
- **Deadlock Prevention**: Never emit a Tauri event while holding an engine lock.
- **Main Thread Requirement**: macOS monitor enumeration and window manipulation must run on the UI main thread (`on_main`).
