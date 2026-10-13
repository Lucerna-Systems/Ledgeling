# Packs Submodule (`src-tauri/src/packs/`)

## Purpose & Scope
The `packs` module handles reading, schema validation, condition expression parsing, and hot-reloading of pet asset bundles.

## Key Submodules
- `manifest.rs`: Pet pack manifest parser (`pet.json`), animation sprite atlases, and schema verification.
- `condition.rs`: Non-Turing-complete declarative boolean grammar (`signal OP value`).
- `install.rs`: Built-in pack extraction, custom pack installation, and diagnostics reports.

## Constraints
- **Packs are Data**: Never execute code from packs. No Lua, no JavaScript, no eval.
- **Graceful Fallbacks**: Missing animations fall back down an established chain (`climb → move → idle`); packs never panic on missing frames.
- **Human-Readable Diagnostics**: Validation failures must return file name, JSON path, problem, and expected format.
