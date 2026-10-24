# Contract between the Rust app and the Vue client

These Tauri commands are the interface the marketplace UI is built on. They are
stable: adding a command or a field is fine; renaming or removing one, or changing
an error `code`, means a deliberate change here first. Types are in
`src/types.ts`. Rust errors of the `{ code, message }` kind put the stable part in
`code` and plain language in `message`; `src/state.ts` `call()` exposes them as
`error` and `errorCode`. Switch on the code, never on the message.

## Online consent (`net`)

| Command | Arguments | Returns |
|---|---|---|
| `net_status` | none | `NetScopeStatus[]`, one per scope (`marketplace`, `analytics`) |
| `net_set` | `scope`, `mode` (`off` \| `session` \| `remembered`) | the new `NetScopeStatus[]`; companion window only |

`net-changed` is emitted with the new status list after every change. Every scope
is `off` until the user chooses otherwise. `analytics` refuses `session`.

## Marketplace installs

| Command | Arguments | Returns |
|---|---|---|
| `gallery_install` | `url`, `expectedSha256`, `galleryId`, `update` | `PackSummary`, or an `InstallError`. Companion window only; needs `marketplace` on |
| `pack_sources` | none | `{ [packId]: PackSource }` for marketplace-installed packs |
| `install_pack` | `path`, optional `expectedSha256` | `PackSummary` or `InstallError` |
| `update_pack` | `path`, optional `expectedSha256` | `PackSummary` or `InstallError`; the version must be higher |
| `export_pack` | `id` | `{ path, sha256 }` |
| `validate_pack_for_publish` | `path` | `Report`; empty of errors means publishable |

`gallery_install` fetches only from the `marketplace` scope's hosts, which are
fixed when the app is built from `LEDGELING_MARKETPLACE_HOSTS` (see
[development.md](development.md#online-server-addresses)). The URL's host must be
on that list; the listing, search and authentication calls
of your server's API are not part of this contract, and a new call to your server
should be made through `net::Net::send`, not from the webview.

## Error codes

Install and download: `unreadable_archive`, `archive_limit`, `not_allowed`,
`no_manifest`, `invalid_pack`, `already_installed`, `not_installed`, `not_newer`,
`bundled`, `hash_mismatch`, `io`.

Network: `offline` (the scope is off), `unavailable` (this build has no server
for it; `net_status` reports `available: false` and `net_set` refuses to turn it
on), `refused` (address not allowed),
`failed`, `too_large`, `cancelled` (the user switched the scope off),
`http_status` (the server answered with an error status).

Pack names (events, signals, actions, classes) are append-only: see
[pack-format-stability.md](pack-format-stability.md).
