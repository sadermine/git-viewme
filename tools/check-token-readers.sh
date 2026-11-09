#!/usr/bin/env bash
# The compiler keeps the token's value inside token.rs, but any file in the crate can call the
# keyring crate itself and read the stored token, because keyring is a dependency of the crate.
# Every use of that crate names it, so a match on the name catches each one.
cd "$(git rev-parse --show-toplevel)" || exit 2
git grep --untracked -nE 'keyring' -- '*.rs' ':!crates/core/src/token.rs'
case $? in
  0) echo "The lines above use the credential store outside crates/core/src/token.rs." >&2; exit 1 ;;
  1) exit 0 ;;
  *) exit 2 ;;
esac
