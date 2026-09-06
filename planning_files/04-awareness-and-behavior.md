# Perch — Awareness & Behavior Design

This is the differentiator. Finch asks you to report your habits; Perch observes the session. The design problem is doing that in a way that feels attentive rather than surveillant, and gentle rather than nagging.

---

## 1. Design principles

1. **Observe, don't interrogate.** No check-in screens, no logging prompts, no "how are you feeling?" modals. Ever.
2. **Ambient, not interruptive.** The pet never steals focus, never opens a dialog, never plays a sound the user didn't opt into. The strongest thing it does is behave differently in the corner of your eye.
3. **Coarse signals only.** The app needs to know you've been heads-down for 90 minutes. It does not need to know in what file. Resolution is deliberately kept low.
4. **Wrong is fine, creepy is not.** A misread signal produces a slightly odd bird. An over-collected signal produces an uninstall.
5. **Fully functional with awareness off.** Awareness is an enhancement layer. Disable everything and you still have a lovely animal on your windows.

---

## 2. Signal collection

### 2.1 Idle time

**Source:** `GetLastInputInfo` (Windows) — returns tick count of last keyboard or mouse input.
**Poll:** 1 Hz.
**Handling:** wraps at ~49 days; clamp negatives to zero.

Derived states:
| State | Threshold | Typical meaning |
|---|---|---|
| `active` | < 30s | At the keyboard |
| `pausing` | 30s – 3m | Reading, thinking, on a call |
| `away` | 3m – 20m | Stepped out |
| `long_away` | > 20m | Lunch, meeting, done for the day |

### 2.2 Foreground application category

**Source:** `GetForegroundWindow` → `GetWindowThreadProcessId` → `QueryFullProcessImageNameW`
**Poll:** 2 Hz, debounced — a category change must persist 5s before it's reported (prevents alt-tab thrash).

**Critical constraint: the executable name is read, mapped to a category, and discarded. The window title is never read at all.**

Window titles contain email subject lines, document names, browser tab titles, customer names, URLs. There is no version of this feature worth that exposure, and no user-facing benefit that requires it.

```
code       → devenv, code, idea*, vim, sublime, rider, xcode
browser    → chrome, firefox, msedge, arc, brave
comms      → slack, teams, discord, zoom, outlook, thunderbird
creative   → photoshop, blender, figma, aseprite, ableton, davinci
media      → vlc, spotify, mpv, netflix (browser is separately handled)
game       → detected via fullscreen + exclusive input heuristics
other      → anything unmatched
```

The mapping table ships as an editable config file so users can extend it locally without an update.

### 2.3 Session rhythm

Tracked entirely in-memory, derived from idle state:

| Concept | Definition |
|---|---|
| **Unbroken stretch** | Minutes since the last `away`-or-longer gap |
| **Break** | An `away` period of ≥ 5 minutes; ends the current stretch |
| **Micro-break** | A `pausing` period of ≥ 90 seconds; noted, doesn't reset the stretch |
| **Session** | Time since app start or since a `long_away` gap |

### 2.4 Environmental context

| Signal | Source | Use |
|---|---|---|
| Hour of day | System clock | Pet sleeps at night, is livelier mid-morning |
| Surface count | World model | Fewer windows → pet has less to do → more idling |
| Fullscreen active | Platform layer | Suspend entirely |
| Battery state | Win32 power API | Reduce activity on low battery |
| Cursor position | Frontend pointer tracking | Proximity reactions |

### 2.5 What is never collected

| Not collected | Why |
|---|---|
| Window titles | Contain document names, email subjects, URLs |
| Keystrokes or key content | Only *that* input occurred, never what |
| Screen contents | No capture, ever |
| Clipboard | No access |
| Browser history / open tabs | Out of scope, permanently |
| File paths | Not needed for any feature |
| Anything persisted to disk beyond aggregate counters | Nothing to leak or subpoena |

---

## 3. The internal model

Rather than mapping signals directly to animations (which produces robotic, obviously-scripted behavior), signals feed two slow-moving scalars that bias state selection.

### Energy `[0,1]`
```
drains slowly while awake, faster when active
recovers while dozing/sleeping
low energy → prefers idle/sleep states, slower movement
modulated by personality.sleepiness and time of day
```

### Mood `[0,1]`
```
rises  — user takes a break after a long stretch
       — user returns from being away
       — user interacts with the pet (click/drag/pet)
       — plenty of surfaces to explore
falls  — very long unbroken stretch (the pet "worries")
       — long stretch of no interaction
       — nowhere to perch (fullscreen for extended periods)
```

Both change on a timescale of minutes, so the pet has continuity — it doesn't snap between personalities. Packs read them as signals (`energy > 0.4`) but never set them directly.

### Why this indirection

A pet that plays `celebrate.png` the instant a 25-minute timer fires reads as a UI element. A pet whose energy has been dropping for two hours, who has been dozing more, and who perks up when you finally stand up reads as *alive*. Same underlying data, entirely different felt experience.

---

## 4. The nudge system

The one place where the pet actively communicates. It must be extremely restrained — this is the feature most likely to cause uninstalls if it's overtuned.

### Nudge ladder

| Trigger | Pet response | Intensity |
|---|---|---|
| 45m unbroken | Becomes visibly more restless — more fidgeting, more relocating | Barely noticeable |
| 75m unbroken | Moves closer to the active window, plays `alert` occasionally | Peripheral |
| 110m unbroken | Sits on the active window's edge, looks at the cursor, plays a soft sound *if sounds enabled* | Noticeable but silent by default |
| Break taken | Plays `happy`, becomes livelier for a few minutes | Reward |
| Return from away | Plays `greet` once | Warm |

### Hard rules

- **Never a popup, toast, or focus steal.** The pet's body language is the entire communication channel.
- **Never text.** No "You've been working for 2 hours!" — that's a notification app wearing a bird costume.
- **Cooldowns on everything.** A nudge reaction cannot re-fire for at least 15 minutes.
- **Escalation caps out.** After the 110m tier there is no further escalation. It doesn't get progressively more desperate.
- **Fully disableable**, and awareness can be turned off wholesale in one toggle.
- **Suppressed in `game` and `media` categories.** Someone watching a film or playing is not "failing to take a break."

### Calibration risk

The realistic failure mode is a pet that feels naggy after week two. Mitigation: ship the thresholds as user-editable settings, default them *conservatively long*, and treat "users report the pet is annoying" as a P1 bug, not a preference issue.

---

## 5. Reaction pipeline

```
Signals (1–2 Hz)
   ↓
Scalar update (energy, mood)
   ↓
Event detection → cooldown filter → priority queue
   ↓
Behavior FSM:  reaction (if any, by priority)  else  weighted transition
   ↓
Target state → action → path request → locomotion step
   ↓
Render frame
```

**Priority bands** (higher wins, and interrupts):
```
100  Survival    — surface_lost, falling, off-screen recovery
 90  Direct      — grabbed, clicked, dragged
 60  Social      — user_returned, cursor_nearby
 50  Nudge       — long_session tiers
 30  Ambient     — idle_long, hour_changed, app_category_changed
  0  Default     — normal weighted FSM wandering
```

Survival always wins. A squirrel whose window just closed stops caring about nudges and deals with falling.

---

## 6. Privacy, stated plainly

This gets its own section because it's a trust product and the settings screen should say roughly this in plain language:

> **Perch runs entirely on your computer.**
> It checks how long since you last touched the keyboard, and which *kind* of application is in front — code editor, browser, chat, and so on.
> It does not read window titles, file names, what you type, or what's on your screen.
> Nothing is sent anywhere. There is no account, no server, and no telemetry.
> You can turn all of this off and keep the pet.

When the AI layer arrives (phase 3):
- Ships **off by default**
- One screen, in plain words, listing exactly what leaves the machine
- Only categories and aggregate durations transmitted — never process names, never titles
- A visible indicator whenever a network call is made
- Full functionality retained with it off

---

## 7. Tuning defaults

Starting values, expected to change after real use:

```
idle.pausing_threshold        30s
idle.away_threshold           3m
idle.long_away_threshold      20m
break.min_duration            5m
nudge.tier1                   45m
nudge.tier2                   75m
nudge.tier3                   110m
nudge.cooldown                15m
energy.drain_per_hour_awake   0.25
energy.recovery_per_hour      0.50
mood.break_reward             +0.15
mood.long_session_penalty     -0.10/hour beyond tier2
app_category.debounce         5s
```

---

## 8. Open questions

- **Should the pet remember across days?** A pet that recognises "you always work late on Thursdays" is charming but requires persistence and moves toward tracking. Leaning yes for aggregate counters only, no for anything resembling a log.
- **Multi-monitor attention.** Should the pet prefer the monitor with the active window, or roam freely? Probably follow, with a setting.
- **Meeting detection.** Camera/mic in use is a strong "don't nudge" signal and is queryable on Windows. Worth adding, needs a privacy note.
- **First-run experience.** The pet's charm takes hours to emerge. Is a slightly accelerated first session worth it, or is that dishonest about what the product is?
