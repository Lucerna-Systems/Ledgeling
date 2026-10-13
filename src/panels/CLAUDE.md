# Frontend Panels (`src/panels/`)

## Purpose & Scope
The `panels` directory houses the primary view screens displayed in the Ledgeling Companion application.

## Key Components
- `DesktopPanel.vue`: Bird's-eye SVG map of active displays, detected windows, and surface perches.
- `SettingsPanel.vue`: Companion settings, startup toggles, developer options, and display assignments.
- `PacksPanel.vue`: Pack management, active pack cards, and installed species gallery.
- `AwarenessPanel.vue`: Daily activity session statistics, nudge ladder parameters, and application category overrides.
- `LabPanel.vue`: Interactive tester for pack creators, live reload diagnostics, and synthetic scenario triggers.

## Architecture
- All panels consume domain composables (`useWorldSnapshot`, `useSettings`, `usePackCatalog`) rather than directly manipulating raw IPC or global singletons.
