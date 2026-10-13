# Frontend Composables (`src/composables/`)

## Purpose & Scope
The `composables` module provides composable Vue stores and action handlers, creating an isolated, reactive boundary between frontend views and underlying IPC/state layers.

## Key Composables
- `useWorldSnapshot.ts`: Exposes reactive pet views, surface geometry bounds, and simulation controls (`pause`, `hide`, `debug`).
- `useSettings.ts`: Read and mutation helpers for application settings (`updateSettings`, `mutateSettings`).
- `usePackCatalog.ts`: Pack discovery, validation reports, installation, and uninstallation.
- `useAssistant.ts`: Active conversation tracking, chat routing, assigned pet context, and file attachments.

## Architecture
- Prefer consuming these composables inside UI views and panels instead of importing global mutable state directly from `src/state.ts`.
