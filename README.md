# nsh-background

Periodic and deferred work while the app is backgrounded, within OS limits.

Part of the Native Shell (NSH) capability set. Same layout as the other SuperNSH repos: a Rust crate, docs in `docs/`, CI on GitHub Actions, and a web status page on GitHub Pages at https://supernsh.github.io/background/.

## Install

In the app's directory:

```sh
nsh add nsh-background
```

`nsh add` needs NSH main or the nightly: the 0.6.0 and 0.6.1 releases do not have it. In a Rust app, `cargo add nsh-background` works with any of them.

TSX imports `nsh-background`.

## Status

The crate and the package exist. `crates/nsh-background` holds the Rust core; `packages/nsh-background` holds the TypeScript API. Together they expose the parity set in [docs/parity.md](docs/parity.md): the background_fetch API (`configure`, `start`, `stop`, `status`, `scheduleTask`, `finish`) and workmanager's registration API, on one in-process engine.

`.github/workflows/ci.yml` runs four jobs, on pushes to `master`, on pull requests, and on manual dispatch: `rust` (ubuntu-latest: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`); `package` (ubuntu-latest: packages the crate, builds it from the package and installs it with `nsh add` on the NSH nightly); `typescript` (ubuntu-latest: installs NSH v0.6.0 through SuperNSH/nsh-builds, then `nsh check` and `nsh test` in `packages/nsh-background`); and `cross-check` (`cargo clippy --target <t> -- -D warnings` for `aarch64-apple-ios-sim` on macos-latest, and for `aarch64-linux-android`, `wasm32-unknown-unknown` and `x86_64-pc-windows-gnu` on ubuntu-latest; all but wasm32 add `--all-targets`, so `tests/android.rs` and `tests/windows.rs` compile under their targets). Run https://github.com/SuperNSH/background/actions/runs/44381445909 (commit `test: correct the Android and Windows bridge assertions`) passed all six jobs: `rust`, `typescript` and the four `cross-check` entries. The two platform tests are compile-only; no runner executes them.

The limits are stated in [docs/parity.md](docs/parity.md): the engine is in-process (OS schedulers pending), native-to-TSX events are delivered by pull (`nextEvent`), a path newer than the last CI run and unverified until CI runs it, and web is unsupported.

## Approach

One `Backend` trait. The first backend is an in-process tokio-timer engine with per-target floors (15 minutes on apple and android, none on windows and linux). Native BGTaskScheduler, WorkManager and Windows `BackgroundTaskBuilder` land behind the same trait. Reasons in [docs/decisions.md](docs/decisions.md).

## Layout

- `crates/nsh-background/`: the Rust crate.
- `packages/nsh-background/`: the TypeScript package.
- `docs/`: `brief.md` (research), `decisions.md` (decided record), `goals.md` (goals, acceptance, edge cases), `parity.md` (per-API status).
- `site/`: static status page, published by `.github/workflows/pages.yml`.
- `.refs/`: gitignored depth-1 clones of the reference repos (`flutter_background_fetch`, `react-native-background-fetch`, `flutter_workmanager`, `nsh`, `biometrics`). Read-only sources for the parity tables.

## Checks

Every compile and test runs in GitHub Actions. Nothing is built on a developer machine.

## Licence

MIT. See [LICENSE](LICENSE).

Repository: https://github.com/SuperNSH/background
