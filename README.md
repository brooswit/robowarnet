# RoboWarNet

A multiplayer RoboTalk arena for the web and Electron. The Rust SpacetimeDB module owns accounts, robot programs, lobby membership, simulations, replays, and ladder results. React renders the arena from server-produced frames. Bun installs dependencies, runs the Vite build, and serves the web build.

## Run locally

Install Bun, Rust with the `wasm32-unknown-unknown` target, and SpacetimeDB CLI 2.15.0. Run the database in its own terminal:

```sh
spacetime start --listen-addr 127.0.0.1:3000
```

From the repository root:

```sh
spacetime publish robowarnet --server local --module-path spacetimedb
bun install --frozen-lockfile
bun run dev
```

Open the URL printed by Vite. Use a second browser profile to join with a second identity. For the built web app, run `bun run build` followed by `bun run start`. For the desktop app, run `bun run desktop`. This launches Electron from source using the same Vite build.

To connect multiple devices, host the same database at a reachable WebSocket URL and set `VITE_STDB_URL` and `VITE_STDB_DATABASE` before building. Use `wss://` for a deployed web app over HTTPS. The browser and Electron each remember their own SpacetimeDB identity token. Accounts persist across launches in that client profile. This version has no shared sign-in or recovery if the token is lost.

## Game rules

- Register a handle, then save a robot in Robot Bay. Only its owner can read or edit its source through the game. Names, rankings, and match replay frames are public.
- Create or join a lobby of up to six players. Each selects one of their saved robots. Once everyone has selected, the host can start the match. The server simulates it and sends the replay. Lobby matches do not affect the ladder.
- New robots and robots with changed source enter the ladder at rank 0. The scheduled reducer compares adjacent ranks from lowest to highest. A lower robot that wins trades places with its opponent. A draw keeps the existing order. After the top pair, the sweep restarts. Ranked matches run without frame capture.
- The first publish seeds 21 archived programs under `legacy`. The Rust interpreter compiles all of them. It uses its own simulation rules and approximates some RoboTalk sensors, interrupts, and weapon behavior. Exact Windows 5.39 match results, classic file import, and the original hardware editor are outside this version.

The archived programs come from [Ian Langworth’s JSRoboWar](https://github.com/statico/jsrobowar/tree/3abb10f4eeab1301f1fe49e09ca40d89eb71551e/robots), which collected them from the original game and Paul Hansen’s tutorial. Original author credits are retained in the program files where available.

## Checks

```sh
cargo test --manifest-path spacetimedb/Cargo.toml
bun run build
bun scripts/smoke.ts
```

Run the smoke script against a disposable local database. It registers two test accounts and exercises account recovery with the same token, bot source isolation, lobby ownership, server replay, and the ladder.

After schema changes, regenerate the checked-in client bindings:

```sh
spacetime generate --lang typescript --module-path spacetimedb --out-dir bindings
```
