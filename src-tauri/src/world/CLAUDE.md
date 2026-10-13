# World Module (`src-tauri/src/world/`)

## Purpose & Scope
The `world` module is responsible for extracting physical desktop geometry into a traversable graph for companions. It operates headlessly without GUI dependencies.

## Key Submodules
- `geometry.rs`: Core coordinate math, Rects, physical vs logical pixel conversion in virtual desktop space.
- `surfaces.rs`: Occlusion subtraction, front-to-back z-order window edge extraction, perimeter clipping, and categorization (`window_top`, `taskbar_top`, `edge_side`, `edge_under`, etc.).
- `traversal.rs`: Graph construction connecting surfaces into `CONTIGUOUS`, `JUMPABLE`, and `AIR` edges.
- `diff.rs`: Scan diffing to detect which surfaces moved, appeared, or disappeared between platform polling ticks.

## Architecture & Constraints
- **Source of Truth**: Rust owns all surface geometry and coordinate positions.
- **Occlusion Subtraction**: Step 2 of the pipeline subtracted z-order occlusion so pets never stand on hidden window frames.
- **Headless & Pure**: Zero Tauri or OS windowing calls in this module. Tested against synthetic window layout rectangles.
