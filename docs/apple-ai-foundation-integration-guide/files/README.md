# Apple Foundation Models provider — integration notes

## What this is

`src/apple_foundation.rs` is an `AiProvider` impl backed by Apple's
on-device model, via the `fm` CLI (macOS 27 Golden Gate+). `src/provider_types.rs`
is a stand-in for your real trait/types — replace it with your actual
`AiProvider`, `Message`, `ToolSpec` definitions and adjust the impl's
signatures to match. If you paste me those, I'll align this exactly
instead of guessing.

Add to your `Cargo.toml`:
```toml
async-trait = "0.1"
once_cell = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["sync", "rt", "macros"] }
```

## Wiring it into the provider registry

Wherever you currently build the list of available providers (probably
something like `providers: Vec<Box<dyn AiProvider>>`), add:

```rust
if let Some(apple) = AppleFoundationProvider::detect(Backend::System) {
    providers.push(Box::new(apple));
}
```

`detect()` returns `None` on non-Mac, Intel Macs, or macOS < 27, so this
is safe to call unconditionally on every platform — it just won't show up
in the provider list where it can't work. It does *not* confirm Apple
Intelligence is toggled on or the ~3GB model has finished downloading;
that surfaces as a normal error on first use, which your UI should show
as "Apple Intelligence isn't ready — check System Settings" rather than a
generic provider failure.

## Product/UX implications worth deciding now

- **Per-pet backend toggle.** Expose `System` vs `PrivateCloudCompute` as
  a setting per pet (or globally) — System is free/unlimited, PCC is
  smarter but quota-limited per Apple ID per day. A pet that silently
  burns your PCC quota is a bad surprise.
- **"Powered by Apple Intelligence" badge.** This is your differentiation
  angle — surface it visibly in the pet's provider picker, not buried in
  settings. It's also a legitimate App Store / marketing line: on-device,
  private, no API key required.
- **Graceful absence.** On Windows/Linux/older macOS/Intel Macs this
  provider just won't appear. Worth a one-line explainer in the UI
  ("Apple Intelligence pets need a Mac with Apple Silicon on macOS 27+")
  rather than the option silently missing with no explanation.
- **Shared-resource contention.** The semaphore in the adapter defaults
  to 1 concurrent `fm` call across *all* pets using this provider. If a
  user has 10 pets all on Apple Intelligence and all "thinking" at once,
  they'll queue rather than pile up — worth surfacing as a subtle
  "waiting for on-device model…" state in a pet's UI rather than looking
  frozen.

## To verify once you're actually on macOS 27 final

- Run `fm --help` and `fm respond --help` — confirm `--pcc`, `--image`,
  `--schema` flag names and whether `--schema` takes a file path or
  inline JSON (this code assumes a path).
- Confirm `fm respond --schema` actually enforces the schema strictly
  enough that `parse_agent_step` won't need a more forgiving parser (e.g.
  markdown-fenced JSON, trailing commentary) — on-device 3B models are
  more prone to schema drift than frontier cloud models, so you may want
  a repair/retry step here in practice.
- Check whether Apple Intelligence's language support restrictions
  (English, French, German, etc. as of macOS 27) matter for your pets'
  personas/localization.
