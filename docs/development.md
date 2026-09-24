# Development workflow

## Branches

| Branch | Role | Gets changes from | Publishes |
|---|---|---|---|
| `main` | testing | squash-merged PRs from work branches | the rolling **pre-alpha** release, on every push |
| `prod` | releases | merge-commit PRs from `main` only | a numbered **alpha**, e.g. `v0.1.0-alpha.3`, on every push |

Nobody commits to `main` or `prod` directly, including agents. Every change goes
through a PR, and the `Verify` workflow (checks on macOS and Windows) has to pass
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
git worktree add ../lupetty-<topic> -b <topic> origin/main
cd ../lupetty-<topic>
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
git worktree remove ../lupetty-<topic>
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
