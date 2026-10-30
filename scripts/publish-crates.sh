#!/usr/bin/env bash
# Publish the workspace to crates.io in dependency order.
#
# Every crate must be published, not just the facade: cargo refuses to upload a
# package whose dependencies do not already resolve from the registry, so a
# `publish = false` dependency makes its dependents unpublishable.
#
# crates.io rate limits are a leaky bucket per user: five brand-new crates as a
# burst then one every ten minutes, while a new version of an existing crate is
# one per minute with a burst of thirty. Rather than assume which regime applies,
# this script publishes back to back and waits only when crates.io says to —
# honouring the retry time it reports when it gives one.
#
# In GitHub Actions the upload is authorised by crates.io trusted publishing: the
# workflow needs `id-token: write` and no token at all.
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

# Parse the moment crates.io says publishing may resume, so a ten-minute new-crate
# refill is waited out exactly once instead of being rediscovered by retries.
retry_after_seconds() {
    local output=$1
    local when
    when=$(sed -nE 's/.*[Pp]lease try again after ([^.]*GMT).*/\1/p' <<<"$output" | head -1)
    if [[ -n "$when" ]] && target=$(date -u -d "$when" +%s 2>/dev/null); then
        local wait=$(( target - $(date -u +%s) ))
        if (( wait > 0 )); then
            # Cap it so a bogus far-future timestamp cannot stall the job.
            (( wait > 1200 )) && wait=1200
            printf '%s\n' "$wait"
            return 0
        fi
    fi
    printf '%s\n' 60
}

retry_publish() {
    local crate=$1
    local attempt=1
    # Each attempt may wait out a full refill, so a low cap still covers the
    # whole workspace; the cap only stops an endless loop on a broken response.
    local max_attempts=40

    while (( attempt <= max_attempts )); do
        local output
        if output=$(cargo publish "${dry_run[@]}" -p "$crate" 2>&1); then
            echo "  published $crate"
            return 0
        fi

        if grep -q "already exists on crates.io index" <<<"$output"; then
            echo "  skipped $crate (version already published)"
            # Nothing was uploaded, so no allowance was spent; the caller must
            # not wait for a refill it does not need.
            return 2
        fi

        if grep -qE "429|too many (crates|requests)|Please try again" <<<"$output"; then
            local wait
            wait=$(retry_after_seconds "$output")
            echo "  rate limited on $crate, waiting ${wait}s (attempt $attempt/$max_attempts)"
            sleep "$wait"
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
status=0
for crate in "${order[@]}"; do
    retry_publish "$crate" || status=$?
    if (( status == 2 )); then
        status=0
        continue
    fi
done

echo "Done."
