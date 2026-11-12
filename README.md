# nsh-background

Periodic and deferred work while the app is backgrounded, within OS limits.

Part of the Native Shell (NSH) capability set. Same layout as the other SuperNSH repos: a Rust crate, docs in `docs/`, CI on GitHub Actions, and a web status page on GitHub Pages at https://supernsh.github.io/background/.

## Status

The crate and the package exist. `crates/nsh-background` holds the Rust core; `packages/nsh-background` holds the TypeScript API. Together they expose the parity set in [docs/parity.md](docs/parity.md): the background_fetch API (`configure`, `start`, `stop`, `status`, `scheduleTask`, `finish`) and workmanager's registration API, on one in-process engine.

`.github/workflows/ci.yml` runs three jobs, on pushes to `master`, on pull requests, and on manual dispatch: `rust` (ubuntu-latest: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`); `typescript` (ubuntu-latest: installs NSH v0.5.0 through SuperNSH/nsh-builds, then `nsh check` and `nsh test` in `packages/nsh-background`); and `cross-check` (`cargo clippy --target <t> -- -D warnings` for `aarch64-apple-ios-sim` on macos-latest, and for `aarch64-linux-android`, `wasm32-unknown-unknown` and `x86_64-pc-windows-gnu` on ubuntu-latest). Per AGENTS.md a claim with no passing run is unverified — the only green run on `master` is the scaffold's, under the one-job workflow this file replaces, so the jobs above are configuration, not a passing run.

The limits are stated in [docs/parity.md](docs/parity.md): the engine is in-process (OS schedulers pending), native-to-TSX event delivery is pending an NSH bridge, and web is unsupported.

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
