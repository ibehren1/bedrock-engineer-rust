#!/bin/sh
# Install the project's git hooks into .git/hooks so they run on commit.
#
# Hooks live under scripts/hooks/ (version-controlled, shared). This copies
# them into the local .git/hooks/ directory, which is where git looks by
# default. Run automatically via the npm "prepare" script on `npm install`.
#
# Intentionally does NOT set core.hooksPath: on managed machines that value
# may be owned by a security tool (e.g. Amazon git-defender, which chains to
# the standard local hooks). We only add our hook alongside it.
set -e

repo_root=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "install-git-hooks: not a git work tree, skipping"
  exit 0
}

src_dir="$repo_root/scripts/hooks"
[ -d "$src_dir" ] || {
  echo "install-git-hooks: no scripts/hooks directory, skipping"
  exit 0
}

# Respect a custom hooks path if one is configured, but warn since our hook
# won't be picked up there automatically.
# --git-common-dir: in a linked worktree .git is a file; the hooks live in the main repo.
hooks_dir="$(git rev-parse --path-format=absolute --git-common-dir)/hooks"
custom=$(git config --local --get core.hooksPath 2>/dev/null || true)
if [ -n "$custom" ]; then
  echo "install-git-hooks: local core.hooksPath is set ($custom); installing there instead"
  hooks_dir="$custom"
fi

mkdir -p "$hooks_dir"
for hook in "$src_dir"/*; do
  [ -f "$hook" ] || continue
  name=$(basename "$hook")
  cp "$hook" "$hooks_dir/$name"
  chmod +x "$hooks_dir/$name"
  echo "install-git-hooks: installed $name"
done
