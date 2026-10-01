#!/bin/sh
# Regenerate the third-party notices bundled with the Tauri app:
#   notices/rust.md      Rust crates, via cargo-about (src-tauri/about.toml + about.hbs)
#   notices/frontend.md  npm production dependencies, via frontend-notices.mjs
#
# Both steps gate on the license accept-lists: cargo-about fails on any crate whose license is
# not accepted, and --strict makes the npm step fail on any unreviewed non-permissive package.
#
# Needs cargo-about: cargo install cargo-about --locked --features cli
set -e

repo_root=$(cd "$(dirname "$0")/../.." && pwd)

if ! cargo about --version >/dev/null 2>&1; then
  echo "notices: cargo-about is not installed." >&2
  echo "notices: install it with: cargo install cargo-about --locked --features cli" >&2
  exit 1
fi

mkdir -p "$repo_root/notices"
(cd "$repo_root/src-tauri" &&
  cargo about generate --workspace --locked --fail about.hbs -o ../notices/rust.md)
echo "notices: wrote $repo_root/notices/rust.md"

node "$repo_root/scripts/notices/frontend-notices.mjs" --strict --out notices/frontend.md
