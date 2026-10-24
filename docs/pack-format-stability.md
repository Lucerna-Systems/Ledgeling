# Pack format stability

Published packs are data that other people's copies of Ledgeling load years from
now. This is what may and may not change without breaking them.

## Append-only names

These closed lists are **append-only**:

- events (`reactions[].on`)
- signals (conditions)
- actions (`behavior.states.*.action`)
- locomotion classes and surface kinds
- clip names and their fallback chains
- manifest field names

Adding a name is always safe: an older Ledgeling ignores what it does not know and
says so in a note. **Renaming or removing a name breaks packs already published**,
so it is not allowed within a format version. If a name is truly wrong, add the
new one, keep the old one working, and stop documenting the old one.

`published_names_are_append_only` in `src-tauri/src/packs/tests.rs` lists every
current event, signal, action and class by name. Removing or renaming one fails
that test; adding one fails it too, until the new name is added to the list in the
same change.

## Format versions

`format` is a whole number, currently `1`. Bump it only for a change an older
build cannot ignore: a required field, a changed meaning, a removed name. A build
refuses a pack declaring a format it does not understand, with a message naming
both numbers. When format 2 exists, the loader should keep reading format 1.

## Limits

The ceilings in `src-tauri/src/packs/limits.rs` may be raised freely. Lowering one
can reject packs already published, so it needs a format bump or a documented
deprecation. The JSON schema carries the same limits, and a test keeps the two in
step.

## Versions of a pack

`version` is `major.minor.patch`, three numbers. A gallery installs an update only
when the version is strictly higher. The pack `id` is its identity: whoever owns an
id in a gallery owns its updates, and enforcing that is the gallery's job, not the
app's.
