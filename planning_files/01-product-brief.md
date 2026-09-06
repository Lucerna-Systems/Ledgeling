# Perch — Product Brief

> **Working codename:** Perch. Placeholder — the name covers the core metaphor (creatures perching on the edges of your windows) and works for birds, squirrels, cats and whatever else gets added later.

---

## 1. One-line description

A desktop companion that lives on the edges of your real windows, moves like the animal it actually is, and quietly notices how your work session is going.

## 2. The problem this addresses

People who work at a computer for long stretches have two unmet wants that currently require two different products:

1. **Ambient company.** Something alive on screen that isn't a notification, isn't demanding, and doesn't cost attention.
2. **Gentle rhythm.** A nudge toward breaks and away from three-hour unbroken grinding, without a nagging timer app that gets muted on day four.

Existing products solve one or the other, on the wrong device, or not at all.

## 3. Competitive landscape (honest version)

| Product | What it does | Where the gap is |
|---|---|---|
| **Finch** (mobile) | Virtual pet bird tied to self-care habits. Huge audience, paid tier, very well executed. | Mobile-only. Progress is **self-reported** — you tell it you drank water. It has no idea what you're actually doing. |
| **Shimeji / Shimeji-ee** (desktop) | Mascot characters that crawl on and interact with your windows. Massive fan modding ecosystem since ~2009. | Purely decorative. Java-era tech, clunky install. No awareness of your work at all. |
| **WindowPet** (desktop, Tauri) | Modern pet overlay, 45+ characters, click-through, cross-platform. | Also purely decorative. Character-skin focused, not behavior focused. |
| **Pomodoro / break timers** | Rigid interval timers. | Zero charm, easily ignored, no sense of presence. |

**The unclaimed slot:** a desktop-native companion whose behavior is driven by *observed* session state rather than self-reporting or a fixed timer — and where the animal actually behaves like that animal.

## 4. What makes this different (the three real bets)

**Bet 1 — Observed, not self-reported.**
The pet reacts to real signals: how long since you touched the keyboard, what kind of app you're in, how long the current unbroken stretch has run. No check-in screens. No logging. You never tell it anything.

**Bet 2 — The desktop is the terrain.**
Your open windows aren't a backdrop, they're the world. A budgie lands on the top edge of your editor. A squirrel climbs up the side of a browser window and runs along the taskbar. Close the window it's standing on and it has to deal with that.

**Bet 3 — Species behave like species.**
Not one animation set reskinned 45 ways. A flier and a climber move through the world under genuinely different rules. This is the thing that's hard to copy and the thing that makes new pets feel like new pets.

## 5. Target audience

- **Primary:** desk workers, developers, students, artists — people who already keep something cosy running (lo-fi streams, aquarium screensavers, Discord idling).
- **Secondary:** the existing desktop-pet / Shimeji modding community, who will make packs if the format is good.
- **Tertiary (later):** the Finch-style wellness audience, if the nudge features mature.

## 6. Scope boundaries

### In scope for v1
- Transparent, always-on-top, per-pet overlay windows
- Real window-edge awareness (perching, walking, climbing on actual window borders)
- Two shipped species proving two locomotion classes: **budgie** (flier) and **squirrel** (climber)
- Local-only session awareness: idle time, foreground app *category*, unbroken-session duration
- A documented, data-only **Pet Pack** format plus a validator
- Windows first, with the architecture kept portable

### Explicitly out of scope for v1
- Any AI / LLM features (phase 3)
- Any backend, account system, or cloud sync
- Notification content interception (OS-policy hard on Windows, effectively closed on macOS)
- Pets performing actions on your machine (phase 4, needs a real permissions model)
- A pack marketplace or in-app store

### Non-goals, permanently
- Not a productivity tracker with charts and streaks. No dashboards, no guilt.
- Not a mascot skin engine. If a pet doesn't move like the animal, it doesn't ship.
- Never sends screen contents, window titles, document names, or keystrokes anywhere.

## 7. Monetization sketch

Deliberately conservative — this is a first shipped product, not a business plan.

| Tier | Contents | Rationale |
|---|---|---|
| **Free** | App, core awareness, 2–3 first-party pets, full Pet Pack support (community packs work fully) | Free packs are the growth engine. Never gate the format. |
| **Pet packs** (paid, one-off) | Extra first-party species with deeper behavior sets | Aligns revenue with the thing that's genuinely expensive to make: animation + behavior design |
| **Supporter / Pro** (later) | AI companion layer, sync, early packs | Only once the AI layer exists and per-call costs need covering |

Distribution: itch.io first (the desktop-pet audience already lives there), GitHub Releases for the free build, Steam only if there's traction.

## 8. Success metrics

**v1 is a success if:**
- It is *finished and shipped*. That is the primary metric.
- 500+ downloads in the first three months
- At least **three community-authored pet packs** exist that the author had no hand in — this is the real proof the format works
- Median session length > 2 hours (people leave it running)
- It functions as portfolio evidence: a real Tauri + Rust + OS-integration app, publicly shipped

**Deliberately not a metric:** revenue. If it earns anything in year one that's upside.

## 9. Key risks

| Risk | Severity | Mitigation |
|---|---|---|
| Per-frame window repositioning is janky or burns CPU | **High** — this is core to the whole illusion | Spike this in M0 before anything else is built. Fallback: single full-desktop overlay window. |
| Scope creep — AI, marketplace, mobile, cloud | **High** — the most likely reason it never ships | Milestones are hard gates. Nothing from phase 3+ enters the repo until v1 is public. |
| Pet Pack format too hard to author | Medium — kills the growth engine | A non-programmer must be able to ship a walking pet in under an hour. Validator + hot reload + worked examples from day one. |
| Antivirus false-positives (window enumeration + always-on-top looks like malware behavior) | Medium | Sign the binary if affordable; publish source; document exactly which APIs are called and why. |
| Fullscreen apps and games break the illusion | Medium | Detect fullscreen and cleanly suspend rather than fighting it. |
| macOS/Linux parity effort | Low for v1 | Windows-only v1, but keep all OS calls behind one trait boundary. |

## 10. The one-sentence pitch

*It's the desktop pet that actually knows you've been staring at the same file for two hours.*
