#!/usr/bin/env bash
# Print the CHANGELOG.md section for one version, for use as release notes.
# Exits non-zero when the changelog has no section for that version, so the
# caller can fall back to GitHub's generated notes.
set -euo pipefail

version=${1:?usage: release-notes.sh <version>}
changelog=${2:-CHANGELOG.md}

[ -f "$changelog" ] || exit 1

awk -v want="## $version" '
  $0 == want { found = 1; next }
  found && /^## / { exit }
  found { print }
' "$changelog" | sed -e '/./,$!d'
