# Feature modules

One folder per Bounded Context, loaded lazily from the route tree. Feature code talks to the
platform only through `usePlatform()` and the ports; it never imports `@tauri-apps/*`, never
checks which runtime it is in, and never uses browser storage (`scripts/check-boundaries.ts`).
