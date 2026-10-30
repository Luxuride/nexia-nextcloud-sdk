#!/usr/bin/env bash
# Publish the workspace to crates.io in dependency order.
#
# Every crate must be published, not just the facade: cargo refuses to upload a
# package whose dependencies do not already resolve from the registry, so a
# `publish = false` dependency makes its dependents unpublishable.
#
# crates.io allows five brand-new crates in a burst, then one every ten minutes,
# per user. A workspace of thirty new crates therefore takes hours, not minutes.
# This script waits and retries on 429 instead of failing the run, so a full
# publish completes unattended.
#
# Usage:
#   scripts/publish-crates.sh            # publish everything, skipping what exists
#   scripts/publish-crates.sh --dry-run  # resolve and package, upload nothing

set -euo pipefail

dry_run=()
if [[ "${1:-}" == "--dry-run" ]]; then
    dry_run=(--dry-run)
fi

cd "$(dirname "$0")/.."

# `cargo publish` writes the packaged .crate under target/package. If target/ was
# created by a root build in the devcontainer, that write fails with EACCES and
# the error names the .crate path rather than the permission problem. Fall back
# to a writable scratch directory instead of failing the whole run.
if ! touch target/.publish-write-test 2>/dev/null; then
    export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/nexia-publish}"
    echo "target/ not writable, packaging under $CARGO_TARGET_DIR"
else
    rm -f target/.publish-write-test
fi

retry_publish() {
    local crate=$1
    local attempt=1
    # A 429 means the bucket is empty; one crate is earned back every 10 minutes,
    # so allow enough attempts to sit out several refills without giving up.
    local max_attempts=40

    while (( attempt <= max_attempts )); do
        local output
        if output=$(cargo publish "${dry_run[@]}" -p "$crate" 2>&1); then
            echo "  published $crate"
            return 0
        fi

        if grep -q "already exists on crates.io index" <<<"$output"; then
            echo "  skipped $crate (version already published)"
            return 0
        fi

        if grep -qE "429|too many (crates|requests)|Please try again" <<<"$output"; then
            echo "  rate limited on $crate, waiting 300s (attempt $attempt/$max_attempts)"
            sleep 300
            attempt=$((attempt + 1))
            continue
        fi

        # Not a rate limit: report the real cause and stop.
        printf '%s\n' "$output" | tail -15
        echo "  FAILED $crate"
        return 1
    done

    echo "  FAILED $crate (still rate limited after $max_attempts attempts)"
    return 1
}

# Dependency order, computed from the manifests rather than hand-maintained.
mapfile -t order < <(python3 scripts/crate-publish-order.py)

echo "Publishing ${#order[@]} crates${1:+ (${1})}"
for crate in "${order[@]}"; do
    retry_publish "$crate"
    # New-crate allowance refills one per ten minutes; waiting the full interval
    # avoids spending retry attempts on a bucket that cannot have refilled.
    if [[ -z "${dry_run[*]:-}" ]]; then
        sleep 600
    fi
done

echo "Done."
