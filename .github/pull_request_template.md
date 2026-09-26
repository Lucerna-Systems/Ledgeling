## Why

<!-- What problem this solves, and why this approach. -->

## What changed

## How it was verified

<!-- Tests added or run, and what you did in the real app. Screenshots for UI changes. -->

## Review checklist

- [ ] I read the whole diff myself, not only a summary of it
- [ ] I ran the change in the app, not only the tests
- [ ] No window titles, screen capture, key state or clipboard access
- [ ] No new network call outside the task adapters; no telemetry
- [ ] Packs are still data only: no code, no eval, and every validation error names the file, JSON path, problem and expectation
- [ ] Nudges are still body language only: no popups, text or focus steals
- [ ] Behavior lives in `engine.rs`, not `app.rs`, and no Tauri event is emitted while the engine lock is held
- [ ] Nothing obviously costs CPU or memory against the performance budget
- [ ] `docs/verification.md` still describes only what has actually been run

## Contributor License Agreement

<!-- Outside contributors only. -->

- [ ] I have read `CLA.md` and agree to it for this contribution and all my future contributions to Ledgeling
