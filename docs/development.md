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

## What CI runs

The Verify workflow (every PR, and in front of every release) only does the work
a change needs. Its `changes` job compares the commit with the **last Release run
that succeeded** on the target branch, which is the last known-good state, and
looks at which folders differ:

| Changed | Runs |
|---|---|
| `src-tauri/`, `packs/`, `schema/`, `sidecar/`, the sidecar or display-test scripts | The three-platform matrix (format, clippy, build, Rust tests, frontend tests) |
| `src/`, `scripts/`, `index.html`, `vite.config.ts`, `tsconfig.json` only | Type-check, build and unit tests, on one platform |
| `package.json`, `package-lock.json`, `.github/workflows/verify.yml` | Both |
| `docs/`, `planning_files/`, `*.md`, `.env.example`, `release.yml` only | Nothing |
| Any file it does not recognise (a new `server/` folder, say) | Everything |

Everything also runs when there is no earlier successful run, or when that run's
commit is no longer in the history. Because the comparison is with the last
*good* run, a red `main` is never hidden: the next change re-runs whatever the
failed commit touched.

Require the single check **`verify-result`** in branch protection (Settings →
Branches). It passes when every job that was needed passed, and when a job was
skipped because nothing it covers changed; requiring the individual matrix jobs
would hang whenever they are skipped. To teach CI about a new folder, edit
`scripts/ci-changes.mjs` and its test.

On a push to `prod`, the whole of verify is skipped when the Release run for that
exact commit on `main` already succeeded, since an alpha is a promotion of a
tested commit. A hotfix pushed straight to `prod` still verifies.

## Online server addresses

Online features (the pet marketplace, anonymous statistics) can only reach the
servers a build was made with. Those are not in the source: they come from two
variables read when the app is compiled.

| Variable | Feature | Value |
|---|---|---|
| `LEDGELING_MARKETPLACE_HOSTS` | Pet marketplace | comma-separated bare host names, e.g. `market.example.com,cdn.example.com` |
| `LEDGELING_ANALYTICS_HOSTS` | Anonymous statistics | the same format |

A bare host name has no `https://`, port or path. Anything else is dropped, and a
feature with no valid host is **unavailable**: it is hidden in the UI, cannot be
switched on, and nothing can connect. A mistake in a setting therefore turns a
feature off; it cannot point the app somewhere unintended. List a CDN host too if
the marketplace redirects downloads to one, because a redirect to any host not on
the list is not followed.

**In CI.** The Release workflow's `build` job runs in the GitHub Environment named
for the channel: `pre-alpha` for pushes to `main`, `alpha` for pushes to `prod`.
Set the two variables under **Settings → Environments → pre-alpha / alpha →
Environment variables** (variables, not secrets: the addresses end up in the
binary anyway). That lets pre-alpha builds talk to a staging server and alphas to
production. The Environments are created for you on first use; add protection
rules there if alphas should need approval. Tests and the Verify workflow set
none, so they run with every online feature unavailable.

**Locally.** Put them in `.env` (see below) and run `npm run dev:app`. Cargo
rebuilds when they change. The host must still be https (the gate never allows
plain http outside the test build), so a local server needs a certificate your
machine trusts.

## Environment variables and secrets

Three kinds of value, kept apart on purpose:

| Kind | Example | Local | CI | Reaches the app? |
|---|---|---|---|---|
| Public build settings | `LEDGELING_MARKETPLACE_HOSTS` | `.env` | Environment **variables** | Yes, compiled in |
| Developer and test settings | `LEDGELING_TEST_ANTHROPIC_MODEL` | `.env` | Environment variables, only on the step that needs them | No |
| Secrets (none yet) | an API token for a script | `.env` | Environment **secrets**, only on the step that needs them | **Never** |

**Never compile a secret into the app.** A value read at build time is baked into
the binary and can be read back out of it. `src-tauri/tests/privacy.rs` fails if
anything but the two public host lists is read at compile time. The app keeps its
own users' keys in the operating system keychain, not in any file.

**Local flow.**

1. `cp .env.example .env` and fill in what you need. `.env` is git-ignored, along
   with `.env.*`; only `.env.example` is tracked.
2. `npm run dev:app` runs `tauri dev` and `npm run build:app` runs `tauri build`
   with `.env` loaded. `npm run env -- <command>` does the same for anything else,
   such as `npm run env -- python3 scripts/benchmark.py`.
3. A variable already set in your shell wins over `.env`. Empty values are
   skipped. The loader prints variable names, never values.
4. Each worktree has its own `.env`. To share one, point at it:
   `LEDGELING_ENV_FILE=~/ledgeling.env npm run dev:app`.

**Adding a secret later.**

1. Add the name to `.env.example` under **Secrets**, with a comment saying what
   needs it. Never a real value.
2. Put the real value in your own `.env`.
3. In CI, add it as an **environment secret** (Settings → Environments, or a
   repository secret if every channel shares it) and pass it with
   `env: NAME: ${{ secrets.NAME }}` on the one step that uses it, as the itch job
   does for `BUTLER_API_KEY`. Not at the workflow or job level.
4. Read it at run time (`std::env::var`, `process.env`, `os.environ`), never with
   `option_env!`.

Rotate a secret that was ever pasted into a chat, a log or a commit; git history
keeps it even after a revert.

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
it has stayed hidden for 60 seconds (`GRACE`, checked every 5 s) and no task is queued, running or
waiting on the user, and the page reports nothing unsaved (`volatile`, true on
the AI providers page where a typed key is not persisted). The frontend writes
`{page, prompt, studio, volatile}` through `companion_save_ui` as it changes
(debounced) to `ui_state.json` in the config folder; `companion_ready` returns
it when the window is recreated. Tasks and conversations live in the Rust store
and are untouched. `RunEvent::ExitRequested` without a code is prevented, so
destroying the companion never ends the app.
