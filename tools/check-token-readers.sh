#!/usr/bin/env bash
# The compiler keeps the token's value inside token.rs, but any file can call the keyring crate
# itself and read the stored token. Rust source can come from any file (#[path], include!) and a
# manifest can add the crate under another name, so every file is searched for the name, except
# the ones that must name it.
cd "$(git rev-parse --show-toplevel)" || exit 2
git grep --untracked -n 'keyring' -- . \
  ':!crates/core/src/token.rs' ':!crates/core/Cargo.toml' ':!Cargo.lock' ':!CLAUDE.md' \
  ':!tools/check-token-readers.sh'
case $? in
  0) echo "The lines above use the credential store outside crates/core/src/token.rs." >&2; exit 1 ;;
  1) exit 0 ;;
  *) exit 2 ;;
esac
