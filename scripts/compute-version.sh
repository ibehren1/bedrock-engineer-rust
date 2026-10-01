#!/bin/sh
# Compute the date-based, git-derived project version: YYYY.MMDD.N
#
#   - The date is the commit date (pinned to the last change, not wall clock).
#   - N is the 0-indexed count of commits on that date, so the first commit of
#     a day is .0, the second .1, etc. This makes the version a pure function
#     of git history — the same commit always yields the same version.
#
# Modes:
#   (default)   Version of HEAD, as already committed. Used by the build
#               (Makefile) so packaged artifacts are versioned reproducibly.
#   --pending   Version for the commit that is ABOUT to be created. The new
#               commit does not exist yet, so `git log` counts only prior
#               commits; today's date is used and the new commit becomes the
#               Nth (0-indexed) of the day. Used by the pre-commit hook.
#
# Prints the version to stdout. Exits non-zero if git history is unavailable.
set -e

DATE_FMT='%Y.%-m%d'

if [ "$1" = "--pending" ]; then
  # The commit being created will be dated "now". Count commits already on
  # today's date reachable from HEAD; the new commit's 0-indexed position is
  # exactly that count (count-before-commit == new commit's index).
  commit_date=$(date +"$DATE_FMT")
  # `git log` fails on an empty repo (no HEAD yet) — treat that as index 0.
  if git rev-parse --verify --quiet HEAD >/dev/null 2>&1; then
    index=$(git log --format=%cd --date=format:"$DATE_FMT" | grep -Fxc "$commit_date" || true)
  else
    index=0
  fi
else
  # Version of HEAD exactly as committed.
  commit_date=$(git log -1 --format=%cd --date=format:"$DATE_FMT")
  count=$(git log --format=%cd --date=format:"$DATE_FMT" | grep -Fxc "$commit_date" || true)
  index=$((count - 1))
fi

echo "${commit_date}.${index}"
