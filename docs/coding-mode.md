# Coding mode

Coding mode is an optional developer workspace. It is one compact window that sits
next to your browser, so you can watch a project change as you or an AI agent
work on it. It is off unless you turn it on, and while it is off none of it loads.

## Turning it on

Go to **Settings → For developers → Developer tools**. Turning it on opens
the coding window straight away, and adds:

- A **Coding mode** entry in the dashboard sidebar, under the main navigation.
- **Coding mode** in the tray menu and the Ledgeling menu (Cmd/Ctrl+Shift+E).
- An **Open coding mode** button next to the switch.
- A **Code here** button in a pet's speech bubble, once that pet has a working
  folder. It opens that folder in coding mode with that pet working in it.

Turning it off closes the window and stops every terminal and run command it
started.

## The window

The first time it opens, the window docks to the right edge of the pet screen, and
after that it reopens wherever you left it. The pin button keeps it above other
windows. Pets can stand on it like any other window.

The header always shows **which pet, which worktree, which branch**, and what is
happening there: *Agent working*, *Needs you*, *Running*. Click it to switch
worktrees. Every tab shows the selected worktree only, so two worktrees never
appear in the same place.

| Tab | What it does |
|---|---|
| **Files** | The worktree's folders. Git-ignored entries are dimmed, changed files carry their git letter, and files flash when something writes them. |
| **Editor** | Monaco, with tabs. Cmd/Ctrl+S saves. A file changed on disk reloads in place unless you have unsaved edits, in which case you choose which version to keep. |
| **Changes** | Every changed file with +/− counts, and a diff against the last commit, inline or side by side. **Follow** jumps to each file as it is written. Stage, unstage, discard and commit are here. |
| **Worktrees** | Your projects and their worktrees. Create one on a new branch and give it a pet, move a pet, remove a worktree (its branch is kept), or open it in Finder/Explorer or Orca. **Add-ons** is at the bottom. |
| **Run** | How the project runs, one button to start it, and the ports it opened. See below. |
| **Terminal** | Real terminals in the worktree, running your own login shell. **Claude Code** starts the `claude` you installed. |
| **Chat** | Ledgeling's assistant, working in this worktree's folder with your usual connection and approvals. |

Cmd/Ctrl+1 to 7 switch tabs.

### One pet, one worktree

Each worktree can have one pet, and each pet can work in only one worktree.
Giving a pet to a new worktree takes it off its old one. A pet shows what its
worktree's agent is doing, using body language only, as everywhere else in
Ledgeling:

- It plays its **busy** clip while Claude Code (or another agent started with the
  Claude Code button) is printing.
- It plays its **attention** clip when the agent rings the terminal bell, which
  agents do when they are waiting for you. Typing in that terminal clears it.

Bindings are saved by pet pack, so they survive a restart.

New worktrees go beside the project, in `<project>.worktrees/<branch>`, never
inside it.

### Running the project

The Run tab reads `.ledgeling/run.json` in the project:

```json
[
  { "name": "Web app", "command": "npm run dev", "cwd": ".", "port": 5173, "url": "http://localhost:5173" }
]
```

`env` (an object of text values) is optional. With no file, the tab offers guesses
based on `package.json` scripts (and the lockfile's package manager), `manage.py`,
`Cargo.toml`, `go.mod`, compose files and Makefile targets. Pick one, write your
own, or press **Ask AI to set this up**:

- **With Claude Code** starts `claude` in a Terminal tab with instructions to write
  the file. It asks before writing, and you can watch it work.
- **With your chat model** puts the same request in the Chat tab.

Validation errors name the file, the JSON path, what was wrong and what was
expected, as pack errors do.

Once a run is going, its port shows up automatically and **Open in browser**
becomes available. The ports list reads the operating system's table of listening
sockets (`lsof` on macOS and Linux, `netstat` on Windows). It never connects to a
port. Ports held by something you started here are marked as yours. Polling runs
only while the Run tab is open or something you started is running.

### Add-ons

The Worktrees tab lists the tools coding mode can use (Git, Node.js, Claude Code,
Orca) and whether each is installed. **Install** runs the bundled script from
`scripts/dev-addons/` in a Terminal tab, where you can watch it:

- **Claude Code** uses Anthropic's official installer.
- **Orca** opens its download page. Its `orca` command is installed from inside
  the Orca app.

Nothing is installed without that click. Orca is optional: worktrees use plain
`git worktree` either way. With Orca installed, **Open in Orca** registers the
project with Orca and opens the worktree's changed files there.

## Not supported: VS Code extensions

Coding mode cannot run VS Code extensions. They need VS Code's extension host
(a Node process), the full `vscode.*` API and often native modules, and Monaco is
only the editor part of VS Code.

The closest partial route would be `@codingame/monaco-vscode-api`. It can load
*web* extensions, TextMate grammars and colour themes. It would add roughly
10–20 MB and a service layer, and fetching extensions from Open VSX would be
network traffic that Ledgeling does not otherwise make. If this is taken up later,
themes and grammars would come first, installed from the Add-ons list rather
than silently.

## How it stays out of the way

- **Frontend.** The window is a separate page (`code.html`), and nothing in the
  pet or dashboard pages imports it. `src/code/boundary.test.ts` enforces this.
  Each tab loads the first time it is opened, so Monaco loads with the Editor or
  Changes tab and xterm with the Terminal or Run tab.
- **Backend.** The service is created when the window first opens, and only while
  Developer tools are on. Before that it holds nothing and runs no threads.
- **Capabilities.** The window's only permission, beyond events, is dragging its
  own title bar (`capabilities/coding.json`). Files, terminals and git go through
  Ledgeling's commands. Those commands answer only the coding window, and they
  check every path against the worktrees of projects you added.
- **Privacy tests.** `tests/privacy.rs` still passes. Coding mode opens no
  sockets, and the capabilities test now checks every capability file.

State lives in `<app config>/coding/workspaces.json`: your projects, pet
bindings, and the window's position and pin.
