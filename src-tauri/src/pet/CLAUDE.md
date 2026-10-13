# Pet Runtime Module (`src-tauri/src/pet/`)

## Purpose & Scope
The `pet` module executes pet simulation, locomotion, pathfinding, and behavior state machines.

## Key Submodules
- `behavior.rs`: Declarative finite-state machine (FSM) evaluating signals and pack conditions.
- `locomotion.rs`: Movement physics and animation frame stepping across locomotion classes (`flier`, `climber`, `walker`, `hopper`).
- `pathfinding.rs`: A* traversal across the surface graph built by `world`.
- `runtime.rs`: Instance lifecycle, active energy/mood meters, and coordinate interpolation `(SurfaceId, t)`.

## Architecture & Constraints
- **Authoritative State**: Rust controls pet coordinates, flip states, and active animation frames.
- **Parametric Position**: Pets sit at `(SurfaceId, t)` with `t ∈ [0, 1]`, so window drag movements automatically carry perched pets.
- **Species vs Class**: Different species are data configurations; only locomotion classes dictate traversal algorithms.
