# What Ledgeling looks at

Ledgeling runs entirely on your computer. There is no account, no server, and no
telemetry. Nothing you do in Ledgeling reaches a network, because the program contains
no code that can reach one.

## Collected

| What | Why | Where it goes |
|---|---|---|
| Seconds since your last keyboard or mouse input | So the pet settles down when you step away and greets you when you come back | Memory only |
| The *kind* of application in front — code, browser, comms, media, creative, game, other | So the pet behaves differently while you work, and stays quiet during films and games | Memory only |
| Position and size of your open windows, and their front-to-back order | This is the floor the pet stands on | Memory only |
| Mouse pointer position | So the pet can react to your cursor and be picked up | Memory only |
| How long your current unbroken stretch of work has been | The nudge ladder, and nothing else | Memory only |
| Your settings, and where each pet was last standing | So it is where you left it after a restart | `settings.json` in Ledgeling's own config folder |

## Never collected

- **Window titles.** Not read, not stored, not requested from the operating system
  at any point. Titles routinely contain email subject lines, document names,
  customer names and URLs, and no feature is worth that.
- **What you type.** Ledgeling knows *that* input happened. It never learns what it was,
  and installs no keyboard hook.
- **Screen contents.** No screenshots, no screen recording permission, ever.
- **The clipboard, your files, your file paths, your browser history or tabs.**
- **Anything sent anywhere.** There is no network code in the build.

## How the application category is worked out

1. Ask the OS which window is in front.
2. Ask which process owns it.
3. Ask that process for its executable name.
4. Look the name up in a table — `code.exe` → `code`, `slack` → `comms` — and keep
   only the category.
5. Throw the name away.

Step 4's table is yours to edit, under **Settings → Advanced → Awareness details**. The window title is never
part of this sequence.

## How this is enforced

`src-tauri/tests/privacy.rs` fails the build if any source file references a
window-title API, a screen-capture API, a keyboard-state API, or anything that can
open a network connection, and if Ledgeling's permission list grants anything beyond
local window events. It is checked by a machine on every test run, not by memory.

## Turning it off

**Settings** has one switch, **Notice my day**, that disables all of it: no idle polling, no
foreground check, no session tracking, no nudges. The pet carries on living on your
windows exactly as before. It just stops noticing your day.
