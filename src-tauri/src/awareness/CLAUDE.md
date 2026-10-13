# Awareness Submodule (`src-tauri/src/awareness/`)

## Purpose & Scope
The `awareness` module observes user activity (idle duration, active foreground application category) and calculates pet energy, mood, and break nudges.

## Key Submodules
- `idle.rs`: Idle timers and input tracking via the `Platform` trait.
- `session.rs`: Continuous stretch timers and micro-break detection.
- `categories.rs`: Mapping process names to high-level application categories (`code`, `browser`, `media`, `game`, etc.).
- `nudges.rs`: The three-tier nudge ladder for gentle posture/break reminders.

## Constraints & Privacy
- **No Window Titles**: Process name detection only. Window titles, active URLs, and document names are never queried or recorded.
- **Subtle Nudges**: Reminders occur strictly via pet body language and movement; no desktop popups or system notifications.
