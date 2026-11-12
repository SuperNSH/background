# Decisions

## Open

None. The three STEP 0 questions are settled below. The work they leave pending is listed in [parity.md](parity.md) under Known gaps.

## Decided

1. **NSH version: 0.5.0 from crates.io.** The crate started on 0.4.0, the released line the rest of the SuperNSH set was on (`SuperNSH/biometrics` pinned `nsh-core = "0.4.0"`), and moved to 0.5.0 with the set on 2026-11-11, the day it shipped. Nothing in this area needs an unreleased `main`-branch feature — the scheduling is our own engine.
2. **Approach: one `Backend` trait, in-process engine first.** The first backend is a tokio-timer engine inside the app's process — the brief's "desktop threads otherwise" line, applied to every target — with per-target floors. Native OS schedulers (BGTaskScheduler, WorkManager, Windows `BackgroundTaskBuilder`) are pending per-platform integration behind the same trait; until one lands, the in-process engine is the behavior for as long as the process lives.
3. **Platforms: apple, android, windows and linux in; web out.** The four are in-process engine targets with pinned floors: apple and android 15 minutes, windows and linux none. Web is an unsupported target and every entry point fails with the coded `NotAvailable` error (`ERR_BACKGROUND_NOT_AVAILABLE`).
4. **Native-to-TSX event delivery waits for an NSH bridge.** Until one exists, the native module's `configure` drops events and the TS class keeps its callbacks for when it lands.
5. **STEP 0 clone outcome (2026-11-09).** All five reference repos cloned at depth 1 into `.refs/`: `fluttercommunity/flutter_workmanager`, `transistorsoft/flutter_background_fetch`, `transistorsoft/react-native-background-fetch`, `longbridge/nsh`, `SuperNSH/biometrics`. No URL failed; no substitution was needed.
6. **Verification split (2026-11-09).** CI is configured to compile every platform file — the `cross-check` matrix covers `aarch64-apple-ios-sim`, `aarch64-linux-android`, `wasm32-unknown-unknown` and `x86_64-pc-windows-gnu`, with `linux.rs` compiled natively by the `rust` job — but no push has run the four-entry matrix yet, so it is unverified until a run is green. The development host can compile and test `apple.rs`, cross-clippy `windows.rs` under `x86_64-pc-windows-gnu` and `wasm32-unknown-unknown`, and nothing more: `linux.rs` and `android.rs` are CI-only, and an `aarch64-unknown-linux-gnu` clippy dies in `ring`'s build script for want of a C cross-compiler. Stated in full in [parity.md](parity.md) under Verification.
