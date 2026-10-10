# Terminal mode

Every pet can have a terminal. Right-click a pet, choose **Terminal** in its
bubble, and a real terminal opens in the same place the Actions and Ask forms
do, running your own login shell in the folder the pet works in. Start **Claude
Code** from the bar and the pet shows when it is working and when it needs you.

Terminal mode is a normal feature. It does not need the `coding` build feature
or Developer tools; coding mode shares the same terminal code
(`src-tauri/src/terminal`, `src/terminal`) and keeps its own gate.

## The pet's folder

A pet's **working folder** is the one on the folder row at the top of its
bubble. Choose one there (or drop the pet onto a folder) and Ledgeling keeps it
as that pet's default: it is saved with the pet in `settings.json` and comes back
after a restart. The terminal opens there, as do the pet's actions and its
coding-mode worktree. A folder that has since been moved or deleted is quietly
forgotten. With no folder, the terminal starts in your home folder.

## In the bubble, and focused

The bubble's terminal shows one tab per terminal in that folder, a **+** for a new
shell, **Claude** to start Claude Code, and a button that opens the same
terminals in the **focused window**: a normal, resizable window with split panes
(⌘D / ⇧⌘D on macOS, Ctrl+Shift+E / O elsewhere), for when you want room.

Terminals live in Rust, not in the bubble. Closing the bubble, or clicking away
from it, does not end them; reopening it picks them up with their recent output.
Escape is left to the program in the terminal (Claude Code uses it to interrupt),
so the bubble's close button and clicking away are how it closes.

## Agents it finds

Ledgeling looks on your login shell's PATH for coding agents and offers each one
it finds as a **Start …** chip: Claude Code (`claude`), Codex (`codex`),
Antigravity (`agy`), Gemini (`gemini`), Grok (`grok`), OpenCode (`opencode`), Cursor
(`cursor-agent`), Copilot (`copilot`), Amp (`amp`), Goose (`goose`) and Aider
(`aider`). Tabs can be renamed by double-clicking them. Only Claude Code reports
its own events (below); the rest are watched by their output.

## What the pet shows

For Claude Code, Ledgeling asks it to report its own events instead of guessing
from its output:

| Claude Code says | The pane shows | The pet |
|---|---|---|
| a prompt was submitted, a tool is running | *Working* / *Using Bash* | its **busy** clip |
| it needs a permission or an answer | *Needs you* | its **attention** clip |
| it finished its turn | *Done* | goes back to normal |

It does this with `claude --settings <file>`, a settings file for that run only:
**your own Claude configuration is never edited.** Each hook runs
`ledgeling --agent-event`, which appends a line to a file in Ledgeling's config
folder (`agents/events/`, mode 0700, deleted with the session). The line holds
the event name, the tool name and the notification text, and nothing else:
never your prompt, file contents or tool input.

A turn that never reports its end (you pressed Escape) stops reading as working
once the terminal has been silent for 20 seconds. Any agent that is not Claude
Code, and any terminal where the events never arrive, falls back to the earlier
behaviour: working while it prints, asking after the terminal bell.

The line format is provider-neutral (`{"event", "tool", "kind", "message"}`), so
another agent that can run a command on its own events can feed it too; only
Claude Code is wired up today.
