# Release plan — against the codebase as of 2026-10-09

**Product name confirmed**: **Ledgeling** (bundle ID `technology.lucerna-systems.ledgeling`).

This turns the 8 Oct 2026 Market Opportunity Report (paid itch.io alpha by
day 21, Steam page in parallel, Desktop Companion Fest 14–18 Jan 2027) into
work packages that match what is actually in the repo. It does not replace
[`00-PLAN.md`](00-PLAN.md) (intent) or [`../docs/verification.md`](../docs/verification.md)
(what has really been run). Read [`../docs/working-on-ledgeling.md`](../docs/working-on-ledgeling.md)
before picking up any package.

Each package says who can do it: **[agent]** is safe for a lower-tier model
working alone in a worktree, **[agent+human]** needs a person to run or judge
something, **[human]** is not code.

## 1. What the report assumes vs. what is true

| Report says | Codebase says | Consequence |
|---|---|---|
| "Cut a release build profile without coding mode" | Already done. Coding mode is the Cargo feature `coding` (off by default); `release.yml` builds with no features, and Settings only shows the Developer tools switch when the backend reports `codingAvailable`. | Not a build task. One check remains: `vite.config.ts` still emits `code.html` into `dist`, and `capabilities/coding.json` exists. Confirm a default build ships nothing usable (WP-2). |
| "Make first-run setup offer a local model first" | Ollama and Apple Intelligence are already grouped as "on this computer" in `src/assistant/ProvidersPanel.vue`. Default provider is `None`. The Ollama live test (`tests/task_connections.rs`) has **never been run**. Apple on-device is macOS-only and needs a very new macOS, so it is irrelevant to Windows buyers. | The work is: verify Ollama end to end, make it the first and default suggestion, and handle "Ollama not installed" gracefully (WP-3). |
| "Build the Windows installer and run it on three machines" | CI already produces an NSIS installer on every push to `main` (pre-alpha release). It has never been installed or run. Windows has had one dev-mode run on one machine at 100% DPI. | The installer exists; what is missing is hands-on testing (WP-1). |
| "8-hour soak and the benchmark on Windows" | `scripts/benchmark.py` and `scripts/soak.py` are **macOS-only**: they default to a `.app` path and shell out to `ps` and `lsof`. | A Windows measurement tool must be written first (WP-4). |
| "One progression hook: trust unlocks squirrel and gecko" | Does not exist. `grep` for trust/unlock finds only approval UI in the assistant. All three species are available from first run. | New feature, new scope. Needs a decision before code (WP-6). |
| "Pet Lab out" of the MVP, but Phase 3 pitches Pet Studio to creators | Pet Lab and Pet Studio ship in the same dashboard (`src/panels/LabPanel.vue`, `src/studio/`). Hiding them would be a code change. | Decide: keep them visible, or hide them behind a setting. Recommended: keep (WP-7). |
| "Name: repo says Lupetty, app says Ledgeling, plan says Perch" | Also true inside the code: GitHub repo `lupetty`, app and bundle `Ledgeling`, env var `PERCH_DIAGNOSTICS`, `.perch-handoff-*` files, `perch-fm-serve.pid`, `AGENTS.md` still says "Perch". | Product name is decided in `tauri.conf.json` (Ledgeling) but not cleared. A rename later is expensive; clear it first (WP-0). |
| "Repo is verified on the wrong platform" | Confirmed. See `docs/verification.md` "Windows — first run only". | WP-1 is the critical path. |
| Test counts in README ("74 Rust tests") | `verification.md` says 149 Rust + 62 UI tests. README and `00-PLAN.md` ("77 tests", "90 Rust tests") are stale. | Doc drift to fix before anything is shown publicly (WP-9). |

## 2. Critical path

```
WP-0 name cleared ──┐
WP-1 Windows hands-on ─► WP-5 fix Windows bugs ─► WP-4 Windows soak + benchmark ─► WP-8 itch.io alpha (day 21)
WP-3 local-model first run ─────────────────────────────────────────────────────────┘
WP-6 progression hook (only if decided) ────────────────────────────────────────────┘
```

Steam page (WP-0 → human work) runs in parallel and does not wait for code.

## 3. Work packages

### WP-0 — Settle and clear the name **[decided]**
Product name confirmed: **Ledgeling** (bundle ID `technology.lucerna-systems.ledgeling`).
Name cleared across Steam, itch.io, domains, and trademark registers. Steam
Direct fee and waiting period accounted for. Code and documentation cleanup of
legacy `perch`/`lupetty` identifiers proceeds in WP-9 with strict backwards
compatibility.
Done: name chosen, cleared, and locked.

### WP-1 — Windows hands-on test pass **[human, with a log file an agent can triage]**
The installer comes from CI: download `Ledgeling_*_windows-x86_64-setup.exe`
from the `pre-alpha` GitHub release (unsigned: SmartScreen → More info → Run
anyway). On three Windows 11 machines vary DPI (100%, 150%), monitor count,
taskbar (auto-hide on/off, left/right). Check the items `verification.md`
lists as "Not checked": installed build, window riding while a window is
dragged, occlusion with overlapping windows, multi-monitor, scaling ≠ 100%,
fullscreen suspension, the pet bubble, the folder drag handoff, the assistant
against real files, tray, autostart, uninstall.
Output: one Markdown file per machine in the scratchpad or an issue per bug,
each with machine, steps, expected, actual. Do not edit `verification.md` yet.
Done when: three machines logged, bugs filed.

### WP-2 — Confirm the default build has no coding mode **[agent]**
Files: `vite.config.ts`, `code.html`, `src-tauri/tauri.conf.json`,
`src-tauri/capabilities/coding.json`, `src/panels/SettingsPanel.vue`.
Steps: build without features (`npm run tauri build`), inspect the bundle for
`code.html`/coding assets, and confirm no window labelled `code` can be opened.
If the page and capability still ship, either drop `code` from the Vite inputs
when not building coding, or accept it as inert and say so in
`docs/verification.md`. Do not touch coding-mode behaviour.
Done when: a short note in the PR states what a default bundle contains, and
`npm run check` and `npm test` pass.

### WP-3 — Local model first **[agent+human]**
Files: `src/assistant/ProvidersPanel.vue`, `src/assistant/state.ts`,
`src-tauri/src/tasks/setup.rs`, `src-tauri/src/tasks/providers.rs`,
`src-tauri/tests/task_connections.rs`.
1. [human] Install Ollama on a Windows machine and a Mac, pull a small model,
   run the ignored `ollama_model_list` test (`cargo test --manifest-path src-tauri/Cargo.toml --test task_connections -- --ignored ollama_model_list`;
   it only lists models, there is no Ollama completion test yet, so add one),
   and try a real multi-step task against a scratch folder. Record the result.
2. [agent] Make local the first and default suggestion in the connect flow
   (Ollama everywhere, Apple Intelligence only where detected); cloud
   providers sit under a clearly separate "uses your own account" heading with
   the existing consent text.
3. [agent] When Ollama is not detected, show one plain-language next step
   (what to install, where), not an error. Existing detection strings are in
   `ProvidersPanel.vue` around line 54.
4. [agent] Add UI tests next to `ProvidersPanel.test.ts` for the ordering and
   the not-installed state.
Constraint: network code may only live in `tasks/providers.rs`, `tasks/search.rs`,
`tasks/commands.rs`, `tasks/apple.rs` (privacy test). Do not add a network call
elsewhere.
Done when: first run offers local before cloud, tests pass, Ollama result recorded.

### WP-4 — Performance and soak on Windows **[agent writes the tool, human runs it]**
`scripts/benchmark.py` and `scripts/soak.py` assume macOS (`ps -Ao`, `lsof`,
`.app` path). Add a Windows path to both (or a sibling `*_windows.py` using
`psutil` or PowerShell `Get-Process`): whole process tree CPU %, working set,
handle count, same JSON output and pass/fail rules, default binary
`src-tauri\target\release\ledgeling.exe` or the installed path via
`PERCH_BENCH_BINARY`. Keep the report text honest about what is not counted.
Then [human] run 10 minutes of benchmark (idle and with a window being
dragged) and the 8-hour soak with at least one sleep/wake, one lock/unlock and
one monitor change. Budget: <0.5% CPU idle, <2% moving, <80 MB RAM one pet.
Done when: result files exist and `verification.md` is updated by a human with
the real numbers, pass or fail.

### WP-5 — Fix the Windows bugs from WP-1 **[agent, one PR per bug]**
Likely areas, from what `verification.md` flags as unverified:
`src-tauri/src/platform/windows.rs` (enumeration, DPI, cloaked windows, taskbar),
`platform/windows_drop.rs` (OLE folder handoff), `platform/pet_ui.rs`
(overlay and bubble window styles). Every fix needs a test that does not need
Windows where possible (extend `src-tauri/tests/timeline.rs` or the unit tests
in `world/`), because the CI Windows leg only runs the suite, not the app.
Do not weaken any assertion to make a test pass.

### WP-6 — Progression hook **[decided, agent to implement]**
**Decision**:
- Budgie is free from first launch.
- Squirrel & Gecko are soft-unlocked via accumulated focused session time
  (awareness layer session/break tracking) and completed assistant tasks.
- An "Unlock all pets" toggle is provided in Settings as an override to eliminate
  day-one refund risk.
- Strictly local storage in settings; never sent anywhere (Constraint 3). No
  telemetry.

Implementation: a counter in `src-tauri/src/settings.rs`, a read-only signal /
settings flag, a lock state in the pack browser (`src/panels/PacksPanel.vue`),
unit tests in `settings.rs`, and a UI test. Authorization: user instruction in
the release report.

### WP-7 — Scope the MVP surface **[decided]**
**Decision**: Keep Pet Studio and Pet Lab visible and accessible in navigation
(Home, Settings, Assistant, History, Packs, Pet Studio, Pet Lab stay visible).
Creator tooling is core to the companion pitch. Linux is not released;
`release.yml` builds only macOS and Windows. Workshop is deferred and out of scope.

### WP-8 — itch.io alpha **[decided]**
**Decision**: Ship an unsigned itch.io alpha with clear download guidance for
both Windows SmartScreen ("More info" → "Run anyway") and macOS Gatekeeper
(right-click Open / quarantine removal guidance). Price set to $4.99 on itch.io.
Pipeline: merging `main` into `prod` publishes an alpha; butler pushes when
`ITCH_ENABLED=true`. Requirements: create itch page, configure `BUTLER_API_KEY`,
`ITCH_TARGET`, `ITCH_ENABLED`, and include SmartScreen/Gatekeeper guidance in
release notes.
Done when: a stranger can buy and install it.

### WP-9 — Documentation drift **[agent, low risk]**
- `README.md`: test count, "Node.js 22.12+" (CI uses Node 24), Windows status.
- `planning_files/00-PLAN.md`: test counts; add a pointer to this file.
- `AGENTS.md`: says product is "Perch" and `npm test` runs "focused Rust tests".
- `CLAUDE.md`: constraint 3 says "no network calls at all", but the opt-in task
  service has network adapters (see `docs/privacy.md`, `tests/privacy.rs`).
  Reword to the true rule. This is a product-claim file, so ask before editing.
- Leftover `perch`/`lupetty` identifiers (`PERCH_DIAGNOSTICS`, `.perch-handoff-*`,
  `perch-fm-serve.pid`, worktree path in `docs/development.md`): rename only
  after WP-0, and keep `.perch-handoff-*` cleanup working for old leftovers.
Never make `docs/verification.md` claim something that was not run.

### WP-10 — Steam page and marketing **[human]**
Coming Soon page, capsule art, trailer from the five "window moments", tags
(Desktop, Idler is deliberately **not** how it is marketed, Cozy, Cute, Pixel
Graphics, Utilities), AI-content disclosure (the assistant), a one-sentence
privacy promise that names the opt-in assistant and Steamworks' own traffic.
Register for Desktop Companion Fest when the page is approved. One code item
sits behind this: the Steam build itself (Steamworks SDK, Workshop) is **not**
started and not needed for the alpha or the page.

### WP-11 — Pet terminal (opt-in) **[decided: gated stretch goal]**
**Decision**: Gated stretch goal for Alpha 1; defer to Alpha 2 if schedule or
ConPTY issues arise.
A standalone terminal attached to a pet, for people who want to run their own
`claude` (or any shell) directly. It is **not** coding mode: no editor,
worktrees, diffs or GitHub views, and those stay behind `--features coding`.
The assistant chat is unchanged and remains the option for API-credit users.
Gate: a person first runs a terminal on Windows (ConPTY: start, resize, paste,
close, busy and attention clips) in week 1-2. If that fails or threatens the
schedule, the package moves to Alpha 2 and does not block the itch.io alpha.
Files: `src-tauri/src/coding/pty.rs` (move out of the `coding` gate; today
`portable-pty` is an optional dependency of that feature),
`src-tauri/Cargo.toml`, a new lazily loaded page for xterm (like `src/bubble`),
`src/code/boundary.test.ts` (extend so the pet and dashboard still never import
xterm), `src/panels/SettingsPanel.vue`, `src-tauri/capabilities/` (a new
capability for the terminal window only; a human reviews it).
Behaviour:
- Off by default, behind a Settings switch with a plain description.
- Right-click a pet, open its terminal, optional button that starts the user's
  installed `claude`. One terminal per pet.
- Pet plays its busy clip while output is flowing and its attention clip on the
  terminal bell; typing clears it. Body language only.
Constraints:
- Busy is inferred from output activity only. Terminal content is never read,
  stored, logged or sent. Add an explicit, narrow rule for this file to
  `tests/privacy.rs`; the user reviews that change, it is not auto-allowed.
- No new network code. Nothing from coding mode's editor, git or GitHub modules.
- Keep it out of the Steam capsule and trailer; it appears only in Settings.
Done when: Windows and macOS runs are logged, the boundary and privacy tests
pass, and `verification.md` is updated by a human with what was actually run.

## 4. Calendar (from the report, today is day 1)

| Week | Dates | Packages |
|---|---|---|
| 1 | 9–15 Oct | WP-0, WP-11 Windows gate test, WP-1 started, WP-2, WP-3 steps 2–4, WP-9 |
| 2 | 16–22 Oct | WP-5, WP-4 run, record clips, itch page draft, signing decision |
| 3 | 23–29 Oct | WP-8 publish, WP-10 page submitted, first Reddit wave |
| 4 | 30 Oct–7 Nov | measure, second wave, decide on 7 Nov |

Kill test, unchanged from the report: fewer than 10 paying buyers and under
about 20 wishlists a day by early November means ship free and move on.

## 5. Rules for whoever works a package

- One package, one branch, one worktree off `origin/main`, one PR into `main`.
  Never commit to `main` or `prod`.
- Do not start a package whose decision is marked **[human decides]** until the
  decision is written into this file.
- If a package would need changing a non-negotiable in `CLAUDE.md`, stop and ask.
- Report test results as run, including failures. Anything not run on real
  Windows hardware is "not run on Windows".
