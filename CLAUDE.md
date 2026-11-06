# git-viewme

## What this is

A Windows desktop app (Tauri 2, Svelte 5) for browsing GitHub repositories and the GitHub Projects linked to them, laid out like Visual Studio Code. The GitHub logic lives in `crates/core` with no Tauri dependency, so a later hosted web version can reuse it and replace only `src/lib/api.ts`.

## Status

Prototype, active. Read-only against GitHub: it sends only `GET` requests and GraphQL queries, never mutations. It holds a real credential: the user's GitHub OAuth token, with the `repo` scope, which can write to every repository the user can. The token lives in the OS credential store (Windows Credential Manager, service `git-viewme`, account `github`).

## Commands

The checks a skill runs before each push are `[[pre_push]]` in `sader-process.toml`; CI runs the same commands (`.github/workflows/ci.yml`). Their prerequisites:

- Rust from `rust-toolchain.toml` (rustup installs it on first use) and Node.js from `.nvmrc`, with `npm ci` run once.
- `cargo clippy -p git-viewme` and the app build need the Windows desk. On Linux the Tauri crate needs the WebKitGTK system packages, so CI does not compile it.

Build a runnable debug app with `npm run tauri build -- --debug --no-bundle`. It runs `npm run build` first and writes `target\debug\git-viewme.exe`. `npm run tauri dev` serves the interface from Vite on port 1420 with live reload.

## Invariants

- `crates/core` does not depend on Tauri. Enforced by `crates/core/Cargo.toml`, which does not list `tauri`, so any Tauri import fails to compile. Check with `cargo tree -p git-viewme-core | grep -i tauri`, which prints nothing.
- The token never reaches the interface. `AccessToken` (`crates/core/src/token.rs`) implements neither `Serialize` nor a `Debug` that shows the value, so a Tauri command cannot return it and a log line cannot print it. All GitHub requests go out from `crates/core/src/github.rs`.
- A `401` from GitHub signs the user out. `GitHub::send` in `crates/core/src/github.rs` deletes the stored token on `CoreError::Unauthorized`, and `Load` in `src/lib/session.svelte.ts` returns the interface to the sign-in screen.
- Each Tauri command in `src-tauri/src/lib.rs` calls one `git-viewme-core` function and returns its result. GitHub logic added to `src-tauri` is lost to the hosted version.
- `src/lib/api.ts` is the only file that imports `@tauri-apps/api`. Check with `grep -rl "@tauri-apps/api" src`.
- Every color is a custom property in `src/app.css`, defined once for light under `:root` and once for dark under `@media (prefers-color-scheme: dark)`. Text tokens keep 4.5:1 contrast against every background token. Check with `grep -rnE '#[0-9a-fA-F]{3,8}\b|(rgb|hsl)a?\(' src --exclude=app.css`, which prints nothing.
- The views in a repository tab come from `VIEWS` in `src/lib/views.ts`. A new view is one entry there and one component in `src/lib/views/`.
- Lint levels live in `[workspace.lints]` in `Cargo.toml` at `deny`, so a warning fails the build without a flag.

## Closed decisions

- OAuth app with the device flow, not a GitHub App: a GitHub App cannot read a personal account's Projects. The app ships a client ID and no client secret.
- `keyring` for the token, with no Linux system libraries, so `crates/core` builds and tests on a plain Linux runner.
- `reqwest` with rustls, not native TLS, for the same reason.
- `tauri.conf.json` sets no window `theme`, so the title bar follows the Windows app mode.
- CI uses the `non-dotnet-checks` deviation of the sadermine CI standard and leaves out compiling `src-tauri` until release.

## Traps

- Sign-in fails with "This build has no GitHub OAuth client ID yet" until the OAuth app is registered. `CLIENT_ID` in `crates/core/src/auth.rs` is a placeholder. Set it to the real client ID, which is not a secret and is committed.
- `cargo` or `rustup` not found, or the wrong Rust version, means the pinned toolchain is not on `PATH`. In a shell where rustup lives outside the default location, set `RUSTUP_HOME` and `CARGO_HOME` to it and put `$CARGO_HOME/bin` first on `PATH` before any cargo command.
- `cargo clippy -p git-viewme-core` passes while `src-tauri` is broken, because it compiles only the core crate. Run `cargo clippy -p git-viewme` too before a pull request.
- `npm run check` fails on any svelte-check warning (`--fail-on-warnings` in `package.json`). Reading a prop at the top level of a component, outside `$derived`, is a warning.
- The Files view relies on `file_tree` returning entries depth first, folders before files (`tree_from_response` in `crates/core/src/files.rs`). A different order hides the wrong rows when a folder collapses.
- The CSP in `tauri.conf.json` applies to the built app only. `npm run tauri dev` can work while a CSP mistake blanks the built window, so test a build.

## Out of scope

Anything not in the prototype spec: writing to GitHub, other version-control hosts, a theme setting, macOS and Linux builds, installers, and a hosted web version. Revisit after the prototype proves the four goals in the spec.
