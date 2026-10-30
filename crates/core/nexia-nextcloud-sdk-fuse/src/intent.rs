//! Distinguishes a file manager browsing a directory from a real bulk read.
//!
//! A file manager lists a directory in milliseconds and then touches every entry in it: it stats
//! them, and for anything it wants to preview it reads a few kilobytes. On a network-backed mount
//! each of those touches can cost a full round trip, so browsing a directory of photos turns into
//! minutes of an apparently-hung application while nothing is actually stuck.
//!
//! The distinction this module draws is between *breadth* and *depth*. A process that reads many
//! different files in quick succession is browsing; one that keeps reading the same file is
//! streaming it, which is what a copy does. Browsing reads are refused (the caller retries later,
//! once the file is hydrated or the preview it wanted exists); streaming reads are always served.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// How long a file access keeps counting as evidence of a directory sweep.
///
/// Long enough to cover one pass over a directory, short enough that a process which browsed
/// minutes ago and now copies a file is not still treated as browsing.
pub const SWEEP_WINDOW: Duration = Duration::from_secs(3);

/// How many *different* files a process must touch inside [`SWEEP_WINDOW`] to count as sweeping.
///
/// A program opening one file touches one; a viewer that reads a file and its sidecar touches two.
/// Three in three seconds is already a directory walk, and every additional file makes the
/// classification more certain.
pub const SWEEP_DISTINCT_FILES: usize = 3;

/// Largest read still considered a peek while a process is sweeping.
///
/// A browser asks for 4-16 KiB to decode a preview. A copy asks the kernel for far more: `cp` on
/// Linux issues 128 KiB reads because that is the kernel's readahead chunk. Classifying on size
/// therefore lets bulk transfers through untouched while catching exactly the speculative reads.
pub const PEEK_MAX_SIZE: u32 = 64 * 1024;

/// How many non-continuing reads of one file are served before refusals begin.
///
/// A process that is *only* browsing still gets to make its first couple of attempts on each file,
/// so opening a single photo from a file manager keeps working; only a sustained sweep gets cut
/// off. This also keeps the rule from firing on a program that happens to touch a few files.
pub const PEEK_GRACE_READS: u32 = 2;

/// Consecutive continuing reads of one file that prove the process is streaming it.
///
/// Once a reader has walked the same file this many times in a row it is copying, not browsing, and
/// its reads are served no matter how small they are or how many other files it touched before.
pub const SEQUENTIAL_TRUST: u32 = 4;

/// What should happen to a read request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadVerdict {
    /// Serve the read.
    Allow,
    /// Refuse the read; the caller is browsing, not copying.
    Refuse,
}

/// What one process has been doing recently.
#[derive(Debug, Default)]
struct ProcessState {
    /// Distinct files touched recently, newest last, pruned to [`SWEEP_WINDOW`].
    files: VecDeque<(u64, Instant)>,
    /// Per-file progress, keyed by inode.
    probes: HashMap<u64, FileProbe>,
}

/// Progress through one file.
#[derive(Debug, Default, Clone, Copy)]
struct FileProbe {
    /// Reads that did not continue the previous one.
    disjoint_reads: u32,
    /// Length of the run of reads that each continued the previous one.
    sequential_run: u32,
    /// Offset just past the last byte read.
    last_end: u64,
    /// When the last read happened.
    last_at: Option<Instant>,
}

impl FileProbe {
    /// Whether `offset` continues straight on from the previous read of this file.
    ///
    /// A reader that restarts from the beginning, or jumps ahead, breaks the run: `cp` walks
    /// forward, while a file manager's peek at offset zero of every file never continues anything.
    fn continues(&self, offset: u64, now: Instant) -> bool {
        let Some(last_at) = self.last_at else {
            return false;
        };
        offset == self.last_end && now.duration_since(last_at) <= SWEEP_WINDOW
    }
}

/// Tracks per-process access patterns to tell browsing from bulk reading.
#[derive(Debug, Default)]
pub struct ReadIntent {
    processes: std::sync::Mutex<HashMap<u32, ProcessState>>,
}

impl ReadIntent {
    /// Classifies a read of `ino` at `offset` for `size` bytes issued by process `pid`.
    pub fn classify(&self, pid: u32, ino: u64, offset: u64, size: u32) -> ReadVerdict {
        self.classify_at(pid, ino, offset, size, Instant::now())
    }

    /// [`Self::classify`] with an explicit clock, so tests can advance time deterministically.
    fn classify_at(
        &self,
        pid: u32,
        ino: u64,
        offset: u64,
        size: u32,
        now: Instant,
    ) -> ReadVerdict {
        let Ok(mut processes) = self.processes.lock() else {
            // Losing the lock must never break reads: an unclassifiable request is served.
            return ReadVerdict::Allow;
        };
        let state = processes.entry(pid).or_default();

        // Forget files this process has not touched within the window.
        while let Some(&(_, at)) = state.files.front() {
            if now.duration_since(at) > SWEEP_WINDOW {
                state.files.pop_front();
            } else {
                break;
            }
        }

        let probe = state.probes.entry(ino).or_default();
        let continues = probe.continues(offset, now);
        if continues {
            probe.sequential_run += 1;
        } else {
            probe.sequential_run = 0;
            probe.disjoint_reads += 1;
            // Record breadth only on the read that starts a file, not on every chunk of it, so one
            // long copy does not look like a sweep through hundreds of files.
            if !state.files.iter().any(|(existing, _)| *existing == ino) {
                state.files.push_back((ino, now));
            }
        }
        probe.last_end = offset.saturating_add(u64::from(size));
        probe.last_at = Some(now);

        // A steady run of continuing reads is a copy or a stream: always serve it.
        if probe.sequential_run >= SEQUENTIAL_TRUST {
            return ReadVerdict::Allow;
        }

        // Nothing suggests browsing yet.
        if state.files.len() < SWEEP_DISTINCT_FILES {
            return ReadVerdict::Allow;
        }

        // Bulk transfers are not peeks, whatever else the process has been doing.
        if size > PEEK_MAX_SIZE {
            return ReadVerdict::Allow;
        }

        // Let the first attempts on each file through, so opening one photo still works.
        if probe.disjoint_reads <= PEEK_GRACE_READS {
            return ReadVerdict::Allow;
        }

        ReadVerdict::Refuse
    }

    /// Drops state for processes that have been idle beyond the window.
    ///
    /// Called opportunistically so the bookkeeping cannot grow without bound as short-lived
    /// processes come and go.
    pub fn prune(&self) {
        let now = Instant::now();
        let Ok(mut processes) = self.processes.lock() else {
            return;
        };
        processes.retain(|_, state| {
            state
                .probes
                .values()
                .any(|probe| probe.last_at.is_some_and(|at| now.duration_since(at) <= SWEEP_WINDOW))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A single file opened once must never be refused, whatever its read shape.
    #[test]
    fn single_file_is_always_served() {
        let intent = ReadIntent::default();
        let now = Instant::now();
        for _ in 0..20 {
            assert_eq!(
                intent.classify_at(1, 10, 0, 4096, now),
                ReadVerdict::Allow,
                "a repeated read of one file must not be treated as browsing"
            );
        }
    }

    /// A copy walks forward through many reads of one file and must always be served.
    #[test]
    fn sequential_copy_is_always_served() {
        let intent = ReadIntent::default();
        let now = Instant::now();
        let mut offset = 0u64;
        for _ in 0..50 {
            assert_eq!(
                intent.classify_at(1, 10, offset, 131_072, now),
                ReadVerdict::Allow
            );
            offset += 131_072;
        }
    }

    /// Small reads spread over many files is what a file manager does, and must be cut off.
    #[test]
    fn sweeping_a_directory_is_refused() {
        let intent = ReadIntent::default();
        let now = Instant::now();
        let mut refused = 0;
        for ino in 10..40u64 {
            // Each file is peeked at several times, always at offset zero, never continuing.
            for _ in 0..5 {
                if intent.classify_at(1, ino, 0, 4096, now) == ReadVerdict::Refuse {
                    refused += 1;
                }
            }
        }
        assert!(
            refused > 0,
            "a sweep of 30 files at 4 KiB was never refused"
        );
    }

    /// The grace period must let the first attempts through so a single open still works.
    #[test]
    fn first_attempts_on_each_file_are_served() {
        let intent = ReadIntent::default();
        let now = Instant::now();
        // Touch several files to enter sweeping state.
        for ino in 0..SWEEP_DISTINCT_FILES as u64 {
            intent.classify_at(1, ino, 0, 4096, now);
        }
        // A new file's first reads are still served.
        for attempt in 1..=PEEK_GRACE_READS {
            assert_eq!(
                intent.classify_at(1, 999, 0, 4096, now),
                ReadVerdict::Allow,
                "attempt {attempt} on a fresh file should be served"
            );
        }
    }

    /// A large read is a bulk transfer and must pass even mid-sweep.
    #[test]
    fn bulk_reads_pass_during_a_sweep() {
        let intent = ReadIntent::default();
        let now = Instant::now();
        for ino in 0..10u64 {
            intent.classify_at(1, ino, 0, 4096, now);
        }
        assert_eq!(
            intent.classify_at(1, 999, 0, PEEK_MAX_SIZE + 1, now),
            ReadVerdict::Allow
        );
    }

    /// Once a process starts streaming one file, its reads must be served again.
    #[test]
    fn streaming_one_file_clears_the_sweep_classification() {
        let intent = ReadIntent::default();
        let now = Instant::now();
        for ino in 10..20u64 {
            for _ in 0..4 {
                intent.classify_at(1, ino, 0, 4096, now);
            }
        }
        // Now read one file straight through, with reads small enough to look like peeks.
        let mut offset = 0u64;
        let mut allowed_after_trust = 0;
        for _ in 0..10 {
            let verdict = intent.classify_at(1, 777, offset, 4096, now);
            offset += 4096;
            if offset > u64::from(SEQUENTIAL_TRUST + 1) * 4096 && verdict == ReadVerdict::Allow {
                allowed_after_trust += 1;
            }
        }
        assert!(
            allowed_after_trust > 0,
            "a continuing sequential read was still refused after proving itself"
        );
    }

    /// Different processes must be classified independently.
    #[test]
    fn processes_are_tracked_separately() {
        let intent = ReadIntent::default();
        let now = Instant::now();
        for ino in 0..20u64 {
            for _ in 0..4 {
                intent.classify_at(1, ino, 0, 4096, now);
            }
        }
        // A different process opening one file must not inherit the sweep classification.
        for _ in 0..10 {
            assert_eq!(
                intent.classify_at(2, 500, 0, 4096, now),
                ReadVerdict::Allow,
                "an unrelated process inherited another process's browsing behaviour"
            );
        }
    }

    /// Evidence must expire, so a process that browsed minutes ago can copy freely.
    #[test]
    fn sweep_evidence_expires() {
        let intent = ReadIntent::default();
        let start = Instant::now();
        for ino in 0..20u64 {
            for _ in 0..4 {
                intent.classify_at(1, ino, 0, 4096, start);
            }
        }
        let later = start + SWEEP_WINDOW + Duration::from_millis(1);
        // A fresh file, long after the sweep, keeps its full grace budget.
        for _ in 0..10 {
            assert_eq!(
                intent.classify_at(1, 999, 0, 4096, later),
                ReadVerdict::Allow,
                "stale sweep evidence still caused a refusal"
            );
        }
    }
}
