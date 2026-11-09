#!/usr/bin/env bash
# The compiler keeps the token's value inside token.rs, but any file can call the keyring crate
# itself and read the stored token. A use in Rust names the crate, and a dependency renamed in a
# manifest names its package, so a match on the name in .rs and .toml files catches each one.
cd "$(git rev-parse --show-toplevel)" || exit 2
git grep --untracked -nE 'keyring' -- '*.rs' '*.toml' \
  ':!crates/core/src/token.rs' ':!crates/core/Cargo.toml'
case $? in
  0) echo "The lines above use the credential store outside crates/core/src/token.rs." >&2; exit 1 ;;
  1) exit 0 ;;
  *) exit 2 ;;
esac
