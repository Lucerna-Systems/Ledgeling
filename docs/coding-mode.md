# Coding mode

Coding mode is an optional developer workspace. It is one compact window that sits
next to your browser, so you can watch a project change as you or an AI agent
work on it. It is off unless you turn it on, and while it is off none of it loads.

Coding mode is **experimental**. It is labelled so in Settings, and it is only in
builds made with the `coding` Cargo feature. Release and pre-alpha builds leave it
out, and the Developer tools setting does not appear in them. It is not part of
what a release promises until it has been run on macOS, Windows and Linux.

## Building with it

```sh
npm run dev:coding                       # tauri dev --features coding
npm run tauri build -- --features coding # a local build that includes it
```

`npm test` and `npm run check` build with the feature, and `check` also runs
Clippy without it, so both kinds of build stay compiling.

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
| **Worktrees** | Your projects and their worktrees. Create one on a new branch and give it a pet, move a pet, remove a worktree (its branch is kept), or show its folder in Finder/Explorer. **Add-ons** is at the bottom. |
| **Run** | How the project runs, one button to start it, and the ports it opened. See below. |
| **Terminal** | Real terminals in the worktree, running your own login shell, split side by side or stacked. **Claude Code** starts the `claude` you installed. |
| **Chat** | Ledgeling's assistant, working in this worktree's folder with your usual connection and approvals. |

### Wide windows

From 760 px wide the window is laid out like VS Code. Narrower, it keeps the
single row of tabs described above.

- **Icon bar** on the far left: Worktrees, Run, and the worktree's pet (its
  side chat), with managing worktrees and add-ons at the bottom. An icon opens
  its view in the side panel; clicking the open one hides the panel. Cmd/Ctrl+B
  also shows and hides it, and its edge can be dragged to resize it.
- **Worktrees** lists every project and worktree, with its pet, branch, what
  is happening there, how many terminals are running, and a pill for each
  address or desktop app they have opened. Click one to switch to it. Under
  each worktree are its **Files**, which open in the side panel with a link
  back to the list, and its **Changes**, which open in the main area.
- **Main area:** Terminals, Editor or Changes, chosen from the tabs above it.

Cmd/Ctrl+1 to 3 open the side views, and 4 to 6 switch the main area. In the
narrow layout Cmd/Ctrl+1 to 7 switch tabs.

### The pet's side chat

The pet given to a worktree, when it is created or later, keeps a running
account of what happens there, in its side chat:

- Claude Code (or a run command) starting, waiting for you, finishing or
  failing with its exit code.
- A terminal starting to serve an address, or a desktop app opening.
- The pet introducing itself when it is given the worktree.

These notes stay in the chat. The pet itself still only uses body language,
and a dot on its icon shows there is something new. Notes last until
Ledgeling quits.

**What's going on?** makes a summary on this computer, with no AI: each
terminal's state and last lines, what is being served, the branch and the
uncommitted changes. Questions typed in the chat go to your chat model, with
that summary attached unless you untick it. A remote model sees it only with
the consent box ticked, as for any request.

### Focus mode

The expand button in the header fills the screen with the wide layout and puts
the terminals in front. The same button returns the window to its compact size
and place, and the window reopens in whichever mode you left it in. Each
worktree keeps its own terminal layout, so switching back finds its terminals
where you left them.

### Split terminals

Every terminal has buttons to split it to the right or downwards, and the new
terminal opens beside the one you last used. Drag the gap between two
terminals to resize them.

| | macOS | Windows and Linux |
|---|---|---|
| Split right | ⌘D | Ctrl+Shift+E |
| Split down | ⇧⌘D | Ctrl+Shift+O |

Run commands started from the Run tab or the open button appear in the grid
too. In the narrow layout the Run tab also shows their output.

### The open button

Each terminal's header has an open button with a chevron:

- When something started in that terminal is **listening on a port**, the
  button shows the port. Click it to open `http://localhost:<port>` in your
  default browser.
- When a **desktop app** started there is running, such as a Tauri or Electron
  app under its dev command, the button shows the app. Click it to bring that
  window forward instead of starting a second copy.

The chevron lists everything the terminal has open, plus ways to start the
project. For a Tauri or Electron project it offers:

- **Show the running dev app**, when another terminal in the worktree has it
  running.
- **Start the dev app** (`npm run tauri dev`, `cargo tauri dev`, the Electron
  `dev` or `start` script), in a new terminal beside this one.
- **Open the production build**, the newest build output
  (`target/release/bundle/macos/*.app`, an `.exe`, an AppImage, or Electron's
  `out/` or `dist/`), with how old it is.
- **Build for preview**, which runs the build in a new terminal below. On
  macOS a Tauri build makes only the `.app`, not the installer.

Entries from `.ledgeling/run.json` are listed too, to start or open.
Bringing an app forward works on macOS and Windows. It is not supported on
Linux yet.

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
every two seconds, and only while the Run tab is open, something you started is
running, or terminals are on screen. The terminals and Run tab share one poll.

### GitHub

The **GitHub** view (a main-area tab in the wide layout, the last tab in the
narrow one; Cmd/Ctrl+7 or 8) shows the repository the worktree pushes to. It
uses your own GitHub CLI (`gh`): your sign-in and your access, so it sees every
repository and board you can. If `gh` is missing or signed out, the view offers
to install it or to sign in with the browser, in a terminal you can watch.

- **Pull requests:** search and filter (open, closed, merged, yours, waiting
  for your review), then read the conversation, the diff, checks and commits.
  Comment, approve or request changes, merge (squash, merge commit or rebase,
  or automatically once requirements pass), mark ready or draft, close, reopen,
  edit the title, labels and assignees, and re-run failed checks. **Check out
  in a worktree** puts the pull request in its own worktree beside the project
  with a free pet, so reviewing never disturbs your work. From there Claude
  Code can review it or address its review comments.
- **Issues:** the same, plus closing as completed or not planned, and **Start
  work**, which makes a worktree on `issue-<n>-<title>` from the default
  branch, optionally with Claude Code already working on it.
- **Boards:** GitHub Projects linked to the repository, then the owner's other
  boards. Cards are grouped by Status (or another single-select field) and can
  be dragged between columns; add a draft card or `#12` to add an issue, archive
  cards, and link a board to the repository. Boards need the `project`
  permission, which **Grant board access** adds.
- The header shows the pull request for the branch on screen, with its checks,
  once the view has been opened. When the branch has none, **Create pull
  request** drafts one from its commits and pushes the branch first.

Links to this repository's issues and pull requests open in the view; other
links open in your browser. Pictures in descriptions are shown as links, since
the window loads nothing from the internet.

### Add-ons

The Worktrees tab lists the tools coding mode can use (Git, Node.js, Claude
Code and the GitHub CLI) and whether each is installed. **Install** runs the bundled script from
`scripts/dev-addons/` in a Terminal tab, where you can watch it:

- **Claude Code** uses Anthropic's official installer.
- **GitHub CLI** uses Homebrew, apt, dnf or pacman, or winget on Windows.

Nothing is installed without that click. Worktrees use plain `git worktree`.

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
