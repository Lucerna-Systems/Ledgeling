# Releasing

`.github/workflows/release.yml` builds and publishes two prerelease channels.
Both run the full check suite (`cargo fmt --check`, `npm run check`,
`npm test`, `npm run test:ui`) on macOS **and** Windows first. If either
platform fails, nothing is published.

| Channel | Trigger | Version | GitHub release |
|---|---|---|---|
| **pre-alpha** | every push to `main`, or *Run workflow* in the Actions tab | `<conf version>-prealpha.<run number>` | one rolling release tagged `pre-alpha`, deleted and recreated on each run |
| **alpha** | pushing a tag `v<conf version>-alpha.<n>` | taken from the tag | a permanent prerelease with generated notes |

`<conf version>` is `version` in `src-tauri/tauri.conf.json`. An alpha tag
whose base version differs from it fails the run, so bump the version before
you tag.

## Cutting an alpha

```sh
git tag v0.1.0-alpha.1
git push origin v0.1.0-alpha.1
```

## Artifacts

| File | For |
|---|---|
| `Perch_<v>_macos-aarch64.dmg` / `.app.tar.gz` | Apple Silicon Macs |
| `Perch_<v>_macos-x86_64.dmg` / `.app.tar.gz` | Intel Macs |
| `Perch_<v>_windows-x86_64-setup.exe` | Windows x64 (NSIS, per-user install) |
| `SHA256SUMS.txt` | checksums of all of the above |

Each build bundles the Claude bridge with a Node binary for its own
architecture. The Intel Mac build runs on an Apple Silicon runner but installs
x64 Node, which is what `prepare-sidecar` copies and what makes npm pick the
x64 agent binary.

Windows-only bundle settings (NSIS target, `.ico`, `node.exe` resource) live
in `src-tauri/tauri.windows.conf.json`, which Tauri merges automatically.

## Not done yet

- **Signing.** Nothing is code-signed or notarized. Gatekeeper and SmartScreen
  both warn, and the release notes tell testers how to get past them.
- **Auto-update.** Deliberately absent: the updater would be a network call,
  and nothing leaves the machine in v1.
