# Background tasks: brief

Periodic and deferred work while the app is backgrounded, within OS limits.

## Options

- No Rust library runs background work on mobile
- BGTaskScheduler: iOS, native
- WorkManager: Android, native
- Desktop: background threads owned by the app

## Recommended path

Native BGTaskScheduler and WorkManager behind one NSH trait. Desktop threads otherwise.

## Edge cases to design for

- The OS decides timing; there is no guaranteed schedule
- Force-quit stops scheduled work on iOS and Android
- Android background starts are restricted on 12+
- iOS gives a short window for background execution

## Research status

Research done 2026-11-09 by web search. Versions and licences are unverified until checked on the registry before pinning. Full catalogue: the research notes for the SuperNSH library set.
