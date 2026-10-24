# Development workflow

## Branches

| Branch | Role | Gets changes from | Publishes |
|---|---|---|---|
| `main` | testing | squash-merged PRs from work branches | the rolling **pre-alpha** release, on every push |
| `prod` | releases | merge-commit PRs from `main` only | a numbered **alpha**, e.g. `v0.1.0-alpha.3`, on every push |

Nobody commits to `main` or `prod` directly, including agents. Every change goes
through a PR, and the `Verify` workflow (checks on macOS, Windows and Linux) has to pass
before the PR can merge.

`prod` is merged, not squashed, so it stays a strict ancestor-chain of `main`.
Squashing there would give the two branches different histories and cause
conflicts on every later promotion.

## Day to day: one worktree per change

Each piece of work, whether done by you or by an agent, gets its own branch in
its own worktree. That keeps parallel agent sessions from interfering with each
other or with your checkout of `main`.

```sh
git fetch origin
git worktree add ../ledgeling-<topic> -b <topic> origin/main
cd ../ledgeling-<topic>
npm ci && (cd sidecar && npm ci)
```

The Claude desktop app can create the worktree for you. Either way, the work
lands on its own branch based on `origin/main`.

When the work is done:

```sh
git push -u origin <topic>
gh pr create --base main
```

Clean up after the merge (GitHub deletes the remote branch automatically):

```sh
git worktree remove ../ledgeling-<topic>
git branch -D <topic>
```

## Reviewing a PR

Generated code gets the same review as code you wrote yourself, or a closer
one. Before merging:

1. **Read the whole diff yourself**, not just the summary of it. `gh pr diff <n>`,
   or the Files tab.
2. **Run it.** Check out the branch (`gh pr checkout <n>` in a scratch worktree)
   and exercise the change in the real app, not only through the tests.
3. **Get a second pass.** `/code-review` in Claude Code looks for correctness
   bugs, and `/security-review` for anything that touches the task layer, packs
   or network adapters.
4. **Go through the PR template checklist.** It lists the product rules from
   `CLAUDE.md` that are easiest to break without noticing.
5. **Check that `docs/verification.md` still tells the truth** after the change.

Merge with **Squash and merge**, and write a squash message that explains why the
change was made.

## Releasing

1. Let the latest `main` sit as a pre-alpha until it has actually been used.
2. Open a PR from `main` into `prod`: `gh pr create --base prod --head main`.
3. Merge it with **Create a merge commit**. The push to `prod` builds, tags and
   publishes the next `v<version>-alpha.<n>`.

To change the base version, bump `version` in `src-tauri/tauri.conf.json`,
`src-tauri/Cargo.toml` and `package.json` together, in a PR to `main`.

Release artifacts are a macOS DMG for Apple Silicon and one for Intel, a
Windows x64 NSIS installer, and `SHA256SUMS.txt`. None of them are signed yet.

## itch.io

The release workflow has an `itch` job that pushes each alpha to itch.io with
butler, in three channels: `macos-aarch64`, `macos-x86_64` and `windows-x86_64`.
Pre-alphas are not pushed. The job is **off** until you turn it on.

To turn it on, go to the repo's Settings → Secrets and variables → Actions and:

1. Add the secret `BUTLER_API_KEY`. Get the key from itch.io under Settings →
   API keys, or by running `butler login` locally.
2. Add the variable `ITCH_TARGET`, set to `<itch user>/<game slug>`.
3. Add the variable `ITCH_ENABLED`, set to `true`.

To turn it off again, delete `ITCH_ENABLED` or set it to anything other than
`true`.

## The companion window is created on demand

A created-but-hidden webview costs roughly 37 MiB, so the companion (`main`) is
declared with `create: false` in `tauri.conf.json` and built by
`companion::ensure` (`src-tauri/src/companion.rs`) from that same config entry.
Every open path (tray, a pet's bubble, `open_assistant`, drops) goes through
`companion::reveal` or `companion::show`; code that merely wants to tell an open
companion something uses `emit_if_open`, and events that must not be lost
(`assistant:open`, `assistant:drop`, `providers:open`) use `companion::send`,
which queues them until the new webview calls `companion_ready`.

Close policy: closing hides the window. The update loop destroys it only after
it has stayed hidden for 10 minutes (`GRACE`) and no task is queued, running or
waiting on the user, and the page reports nothing unsaved (`volatile`, true on
the AI providers page where a typed key is not persisted). The frontend writes
`{page, prompt, studio, volatile}` through `companion_save_ui` as it changes
(debounced) to `ui_state.json` in the config folder; `companion_ready` returns
it when the window is recreated. Tasks and conversations live in the Rust store
and are untouched. `RunEvent::ExitRequested` without a code is prevented, so
destroying the companion never ends the app.
