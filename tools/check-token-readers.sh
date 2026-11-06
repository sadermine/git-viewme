#!/usr/bin/env bash
# Only the token entry of [[guarded]] in sader-process.toml reads the OAuth token, so a reader in
# any other file would run at prototype with no security review. This matches by name; #14 moves
# the rule into the compiler.
cd "$(git rev-parse --show-toplevel)" || exit 2
git grep -nE 'load_token|secret[(]|get_password|keyring' -- crates src-tauri ':!*.toml' \
  ':!crates/core/src/token.rs' ':!crates/core/src/github.rs' ':!crates/core/src/auth.rs'
case $? in
  0) echo "The lines above read the token outside token.rs, github.rs and auth.rs." >&2; exit 1 ;;
  1) exit 0 ;;
  *) exit 2 ;;
esac
