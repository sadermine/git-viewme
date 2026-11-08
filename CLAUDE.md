# git-viewme

## What this is

A Windows desktop app (Tauri 2, Svelte 5) for browsing GitHub repositories and the GitHub Projects linked to them, laid out like Visual Studio Code. The GitHub logic lives in `crates/core` with no Tauri dependency, so a later hosted web version can reuse it and replace only `src/lib/api.ts`.

## Status

Prototype, active. Its one write to GitHub is moving a card between lanes of a project, which sets the item's `Status` field to the lane's option. Every other request is a `GET` or a GraphQL query. It holds a real credential: the user's GitHub OAuth token, with the `repo` and `project` scopes, which can write to every repository and project the user can. The token lives in the OS credential store (Windows Credential Manager, service `git-viewme`, account `github`). The OAuth app's client ID is not a secret and lives in `%APPDATA%\git-viewme\settings.json`.

## Commands

The checks a skill runs before each push are `[[pre_push]]` in `sader-process.toml`; CI runs the same commands (`.github/workflows/ci.yml`). Their prerequisites:

- Rust from `rust-toolchain.toml` (rustup installs it on first use) and Node.js from `.nvmrc`, with `npm ci` run once.
- `cargo clippy -p git-viewme` and the app build need the Windows desk. On Linux the Tauri crate needs the WebKitGTK system packages, so CI does not compile it.

Build a runnable debug app with `npm run tauri build -- --debug --no-bundle`. It runs `npm run build` first and writes `target\debug\git-viewme.exe`. `npm run tauri dev` serves the interface from Vite on port 1420 with live reload.

## Invariants

- `crates/core` does not depend on Tauri. Enforced by `crates/core/Cargo.toml`, which does not list `tauri`, so any Tauri import fails to compile. Check with `cargo tree -p git-viewme-core | grep -i tauri`, which prints nothing.
- The token never reaches the interface. `AccessToken` (`crates/core/src/token.rs`) implements neither `Serialize` nor a `Debug` that shows the value, so a Tauri command cannot return it and a log line cannot print it. All GitHub requests go out from `crates/core/src/github.rs`. Only `token.rs`, `github.rs` and `auth.rs`, the `token` entry of `[[guarded]]` in `sader-process.toml`, read the token, so a change to any reader runs at gold. `tools/check-token-readers.sh`, run by `[[pre_push]]` and CI, fails on `load_token`, `secret(`, `get_password` or `keyring` anywhere else. It matches by name, so an alias it does not list gets past it until #14 lands.
- A `401` from GitHub signs the user out. `GitHub::send` in `crates/core/src/github.rs` deletes the stored token on `CoreError::Unauthorized`, and `Load` in `src/lib/session.svelte.ts` returns the interface to the sign-in screen.
- The only GitHub write is `SET_STATUS` in `crates/core/src/board.rs`, sent by `GitHub::move_card`. Check with `grep -rn 'mutation(' crates src-tauri/src`, which prints only that one. `card_move_request` builds its variables, tested from a recorded board in `crates/core/tests/board.rs`.
- No card moves to **No status**, and that lane shows only when an item has no status. `CardMove.option_id` in `crates/core/src/lib.rs` is a required option ID, so a move cannot clear `Status`, checked by `card_move_without_an_option_is_refused_so_no_card_moves_to_no_status`. `ProjectView.svelte` offers the lane neither as a drop target nor in **Move to**.
- A token without the `project` scope keeps working for reads and is not deleted. GitHub refuses a move from it with `INSUFFICIENT_SCOPES`, which `error_for_graphql` in `crates/core/src/error.rs` maps to `CoreError::MissingScope`, and `GitHub::send` deletes the token on `CoreError::Unauthorized` only. The Project view then asks the user to sign in again. Tested by `error_for_graphql_insufficient_scopes_is_missing_scope` in `crates/core/tests/errors.rs`.
- A move shows at once and is undone when GitHub refuses it. `move` in `src/lib/views/ProjectView.svelte` moves the card, sends the mutation and puts the card back at its old place on any error. A card is not moved again until GitHub answers.
- Each Tauri command in `src-tauri/src/lib.rs` calls one `git-viewme-core` function and returns its result. GitHub logic added to `src-tauri` is lost to the hosted version.
- The repository holds no OAuth client ID. Each user registers their own OAuth app and enters its client ID on the sign-in screen; `crates/core/src/settings.rs` saves it in `settings.json` in `dirs::config_dir()/git-viewme`, found by core itself. Sign-out and a `401` delete the token only, never the client ID. Check with `grep -rnE 'Ov23[A-Za-z0-9]{16}|Iv1\.[0-9a-f]{16}' . --exclude-dir=node_modules --exclude-dir=target --exclude-dir=.git`, which prints nothing.
- `src/lib/api.ts` is the only file that imports `@tauri-apps/api`. Check with `grep -rl "@tauri-apps/api" src`.
- Every color is a custom property in `src/app.css`, defined once for light under `:root` and once for dark under `@media (prefers-color-scheme: dark)`. Text tokens keep 4.5:1 contrast against every background token. Check with `grep -rnE '#[0-9a-fA-F]{3,8}\b|(rgb|hsl)a?\(' src --exclude=app.css`, which prints nothing.
- The views in a repository tab come from `VIEWS` in `src/lib/views.ts`. A new view is one entry there and one component in `src/lib/views/`.
- Pull requests never reach the issues list, although GitHub's issues endpoint returns them. `issues_from_nodes` in `crates/core/src/issues.rs` drops every entry with `pull_request`, checked by `issues_from_pages_leaves_out_pull_requests` in `crates/core/tests/issues.rs`.
- The Pull requests view lists each pull request in the chosen state at most once. `list_pull_requests` in `crates/core/src/pulls.rs` reads pages of 100 and stops at the first page with fewer than 100 items. A pull request that moves to the top during paging shifts every row below it, so one row comes back on two pages and the moved one is on neither until the next load. `pull_requests_from_nodes` keeps one copy of each number, because a repeated number breaks the view's keyed list (`each_key_duplicate`). Tested in `crates/core/tests/pulls.rs`.
- A merged pull request is shown as merged, not closed. GitHub reports it as `state: closed`, and only `merged_at` tells it apart. `pull_requests_from_nodes` in `crates/core/src/pulls.rs` folds `state`, `merged_at` and `draft` into one `PullRequestStatus`. `pull_requests_from_pages_tells_merged_from_closed_without_merging` in `crates/core/tests/pulls.rs` checks it.
- Lint levels live in `[workspace.lints]` in `Cargo.toml` at `deny`, so a warning fails the build without a flag.

## Closed decisions

- OAuth app with the device flow, not a GitHub App: a GitHub App cannot read a personal account's Projects. The app ships no client ID and no client secret, so no user signs in through someone else's app.
- The Tauri `identifier` is `git-viewme`, and Tauri names every app folder after it, such as the WebView2 data in `%LOCALAPPDATA%\git-viewme\EBWebView`. It matches the settings folder core picks. No `appDirectoriesOverride`.
- `keyring` for the token, with no Linux system libraries, so `crates/core` builds and tests on a plain Linux runner.
- `reqwest` with rustls, not native TLS, for the same reason.
- `tauri.conf.json` sets no window `theme`, so the title bar follows the Windows app mode.
- CI uses the `non-dotnet-checks` deviation of the sadermine CI standard and leaves out compiling `src-tauri` until release.

## Traps

- An OAuth app with **Expire user access tokens** ticked signs in fine, then sends the user back to sign-in every 8 hours, because GitHub's token expires and the app reads no refresh token. Leave it unticked; the sign-in screen says so.
- GitHub answers an unknown client ID at `/login/device/code` with `404` and `{"error":"Not Found"}`, not an OAuth error code. `sign_in` in `crates/core/src/auth.rs` maps that, and the OAuth codes for a bad or device-flow-disabled app, to `CoreError::ClientIdRejected`.
- `cargo` or `rustup` not found, or the wrong Rust version, means the pinned toolchain is not on `PATH`. In a shell where rustup lives outside the default location, set `RUSTUP_HOME` and `CARGO_HOME` to it and put `$CARGO_HOME/bin` first on `PATH` before any cargo command.
- `cargo clippy -p git-viewme-core` passes while `src-tauri` is broken, because it compiles only the core crate. Run `cargo clippy -p git-viewme` too before a pull request.
- `npm run check` fails on any svelte-check warning (`--fail-on-warnings` in `package.json`). Reading a prop at the top level of a component, outside `$derived`, is a warning.
- The Files view relies on `file_tree` returning entries depth first, folders before files (`tree_from_response` in `crates/core/src/files.rs`). A different order hides the wrong rows when a folder collapses.
- `Pagination with the page parameter is not supported for large datasets, please use cursor based pagination` (a `422`) comes from a REST list fetched with `page` past 99. Fetch every REST list with `GitHub::get_pages` in `crates/core/src/github.rs`, which follows the cursor in the `Link: rel="next"` header. A `page` loop works on small repositories and fails only on large ones.
- `fatal: not a git repository: /mnt/c/...` from a `[[pre_push]]` check means pwsh ran WSL's `bash`, which comes first on the Windows `PATH`. Run bash checks with Git for Windows: `& "C:\Program Files\Git\bin\bash.exe" tools/check-token-readers.sh`.
- Dragging a card does nothing in the built app on Windows while the window's `dragDropEnabled` is on, because Tauri's file-drop handler takes the drag events from WebView2. `tauri.conf.json` sets it to `false`; the app takes no file drops.
- The CSP in `tauri.conf.json` applies to the built app only. `npm run tauri dev` can work while a CSP mistake blanks the built window, so test a build.

## Out of scope

Anything not in the prototype spec: writing to GitHub beyond moving a card between lanes (reordering cards within a lane, editing other fields, adding, archiving or deleting cards), other version-control hosts, a theme setting, macOS and Linux builds, installers, and a hosted web version. Revisit after the prototype proves the four goals in the spec.
