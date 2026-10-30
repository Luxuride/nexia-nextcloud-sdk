//! Prevents a file manager's directory sweep from downloading every original.
//!
//! Listing a directory is cheap — `readdir` answers 625 entries in a few milliseconds — but a file
//! manager then touches every entry it lists, and on a network-backed mount every touch of a file
//! whose bytes are not local is a full round trip. A sweep of a photo directory therefore turns
//! into minutes of an apparently-hung window while nothing is actually stuck.
//!
//! Two facts drive the design, both measured against a live mount:
//!
//! * Read size cannot separate a sweep from a copy. The kernel coalesced a file manager's previews
//!   and an ordinary `cp` into identical 128 KiB reads, so no size threshold can tell them apart.
//! * Only some processes browse. `cp`, an archiver, an editor or a script reads what it wants and
//!   stops; it is the GUI file manager that samples a whole directory. So the guard is confined to
//!   processes named in [`BROWSER_COMMS`] and never applies to anything else.
//!
//! Within a browsing process, a read is refused only when it is *not* reading content through:
//!
//! * a read that continues the run already in progress on the same file is served, which is what a
//!   copy or a viewer does once it has committed to a file;
//! * a refusal budget per file guarantees the bytes are served after a bounded number of retries,
//!   so no file can ever become permanently unreadable, and a copy inside the file manager still
//!   completes;
//! * evidence of sweeping expires, so browsing and then copying in the same process works.
//!
//! The guard is also only consulted for files whose bytes are not local yet. Once a file is
//! hydrated a read costs nothing and is always served.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Process names whose speculative reads are subject to the guard.
///
/// Matched against `/proc/{pid}/comm`, exactly as [`crate::fs`] does for thumbnailers, because that
/// is the only name a process cannot forge and the only one that does not require walking
/// ancestors (which previously misclassified unrelated processes).
///
/// Confined to the GUI file managers that sweep directories. Nothing here ever matches `cp`,
/// `rsync`, `tar`, a viewer or a script, so those are never throttled.
pub const BROWSER_COMMS: &[&str] = &[
    // GNOME Files. The one that sweeps a photo directory to build a grid of previews.
    "nautilus",
    // Thunar and Nemo are the same shape of program and would behave identically.
    "thunar", "nemo", // KDE's file manager.
    "dolphin",
];

/// How long a read run stays valid as evidence that a reader is working through one file.
///
/// A copy reads continuously, so its next read follows almost immediately. The window only has to
/// absorb scheduling delays, not a user's thinking time.
pub const RUN_WINDOW: Duration = Duration::from_secs(5);

/// How long a file access keeps counting as evidence of a directory sweep.
pub const SWEEP_WINDOW: Duration = Duration::from_secs(3);

/// How many *different* files a browsing process must touch inside [`SWEEP_WINDOW`] to count as
/// sweeping.
///
/// A single preview touches one file. Three in three seconds is already a directory walk.
pub const SWEEP_DISTINCT_FILES: usize = 3;

/// The largest read a file manager may make while sweeping a directory without being treated as a
/// probe rather than as content access.
///
/// A sweep samples each entry with a handful of kilobytes - 4 KiB, 8 KiB, 16 KiB, whatever the
/// sniffing library wants - while anything that actually reads a file (a copy, an unarchiver, a
/// viewer) asks for real chunks: 128 KiB or more. The size is therefore what separates "wants a
/// hint about this file" from "wants these bytes", and it is the only signal that can vouch for a
/// copy's *first* read, which no run evidence can.
pub const PROBE_MAX: u32 = 16 * 1024;

/// What should happen to a read request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadVerdict {
    /// Serve the read.
    Allow,
    /// Refuse the read: the caller is sampling a directory, not reading content.
    Refuse,
}

/// Returns `true` when `pid` is one of the file managers listed in [`BROWSER_COMMS`].
pub fn is_browser_process(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        let Ok(comm) = std::fs::read_to_string(format!("/proc/{pid}/comm")) else {
            return false;
        };
        let comm = comm.trim();
        if comm.is_empty() {
            return false;
        }
        BROWSER_COMMS.contains(&comm)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        false
    }
}

/// Progress through one file, per reading process.
#[derive(Debug, Default, Clone, Copy)]
struct FileProbe {
    /// Where the current run of continuing reads has reached.
    last_end: u64,
    /// When that run last advanced.
    last_at: Option<Instant>,
}

impl FileProbe {
    /// Whether `offset` continues straight on from the previous read of this file.
    ///
    /// This is the "reading content through" signal: a copy and an unarchiver walk a file forward,
    /// while a sweep reads offset zero of each file once and never continues anything.
    fn continues(&self, offset: u64, now: Instant) -> bool {
        let Some(last_at) = self.last_at else {
            return false;
        };
        offset == self.last_end && now.duration_since(last_at) <= RUN_WINDOW
    }
}

/// What one process has been doing recently.
#[derive(Debug, Default)]
struct ProcessState {
    /// Distinct files touched recently, newest last, pruned to [`SWEEP_WINDOW`].
    files: VecDeque<(u64, Instant)>,
    /// Per-file progress, keyed by inode.
    probes: HashMap<u64, FileProbe>,
}

/// Decides whether a browsing process's read of an unhydrated file belongs to a directory sweep.
///
/// Only consulted for processes in [`BROWSER_COMMS`], and only for files whose bytes are not local.
#[derive(Debug, Default)]
pub struct ReadIntent {
    processes: std::sync::Mutex<HashMap<u32, ProcessState>>,
}

impl ReadIntent {
    /// Classifies a read of `ino` by `pid` at `offset` for `size` bytes.
    ///
    /// `browser` must come from [`is_browser_process`]; a process that is not a file manager is
    /// never throttled, so callers that know their reader is not one may skip the lookup.
    pub fn classify(
        &self,
        pid: u32,
        browser: bool,
        ino: u64,
        offset: u64,
        size: u32,
    ) -> ReadVerdict {
        self.classify_at(pid, browser, ino, offset, size, Instant::now())
    }

    /// [`Self::classify`] with an explicit clock, so tests can advance time deterministically.
    fn classify_at(
        &self,
        pid: u32,
        browser: bool,
        ino: u64,
        offset: u64,
        size: u32,
        now: Instant,
    ) -> ReadVerdict {
        // Nothing but a file manager is ever subject to this rule.
        if !browser {
            return ReadVerdict::Allow;
        }

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
        if !continues && !state.files.iter().any(|(seen, _)| *seen == ino) {
            // Record breadth only when a file is first touched, never on every chunk of it, so one
            // long copy does not look like a sweep.
            state.files.push_back((ino, now));
        }
        probe.last_end = offset.saturating_add(u64::from(size));
        probe.last_at = Some(now);

        // Reading one file through is content access, not sampling: always serve it.
        if continues {
            return ReadVerdict::Allow;
        }

        // Nothing suggests browsing yet: a single preview or a two-file look is ordinary use.
        if state.files.len() < SWEEP_DISTINCT_FILES {
            return ReadVerdict::Allow;
        }

        // A copy asks for real chunks; a sweep samples a file a few kilobytes at a time. Size is
        // what separates "wants a hint about this file" from "wants these bytes", and it is what
        // keeps a copy started inside the file manager working: a copy is served from its very
        // first read, when no run evidence exists yet, and is never refused at any point.
        if size > PROBE_MAX {
            return ReadVerdict::Allow;
        }

        ReadVerdict::Refuse
    }

    /// Drops all bookkeeping for one process, e.g. once it exits.
    pub fn forget(&self, pid: u32) {
        let Ok(mut processes) = self.processes.lock() else {
            return;
        };
        processes.remove(&pid);
    }

    /// Drops bookkeeping for processes that have been idle beyond the window.
    ///
    /// Called opportunistically so the map cannot grow without bound as processes come and go.
    pub fn prune(&self) {
        let now = Instant::now();
        let Ok(mut processes) = self.processes.lock() else {
            return;
        };
        processes.retain(|_, state| {
            state.probes.values().any(|probe| {
                probe
                    .last_at
                    .is_some_and(|at| now.duration_since(at) <= SWEEP_WINDOW)
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing but a file manager may ever be throttled.
    #[test]
    fn non_browser_processes_are_never_refused() {
        let intent = ReadIntent::default();
        let now = Instant::now();
        for ino in 0..200u64 {
            assert_eq!(
                intent.classify_at(1, false, ino, 0, 131_072, now),
                ReadVerdict::Allow,
                "a process that is not a file manager was refused"
            );
        }
    }

    /// The sweep this guard exists for: one sample of each of many files, never continuing.
    #[test]
    fn sampling_a_directory_is_refused() {
        let intent = ReadIntent::default();
        let now = Instant::now();
        // The first two files establish the sweep.
        for ino in 0..SWEEP_DISTINCT_FILES as u64 - 1 {
            intent.classify_at(1, true, ino, 0, PROBE_MAX, now);
        }
        let mut refused = 0;
        for ino in 10..40u64 {
            if intent.classify_at(1, true, ino, 0, PROBE_MAX, now) == ReadVerdict::Refuse {
                refused += 1;
            }
        }
        assert_eq!(
            refused, 30,
            "a sweep of 30 files was not refused, so it would still download every original"
        );
    }

    /// A copy must be served from its very first read, while a sweep is in progress: it asks for
    /// real chunks, so it never looks like a probe, and it can never be refused.
    #[test]
    fn a_copy_is_served_from_its_first_read() {
        let intent = ReadIntent::default();
        let now = Instant::now();
        // Establish a sweep first, the worst case for a copy.
        for ino in 0..10u64 {
            intent.classify_at(1, true, ino, 0, PROBE_MAX, now);
        }
        let mut offset = 0u64;
        for read in 0..40 {
            assert_eq!(
                intent.classify_at(1, true, 777, offset, 131_072, now),
                ReadVerdict::Allow,
                "read #{read} of a copy started during a sweep was refused"
            );
            offset += 131_072;
        }
    }

    /// A sweep must never collect: it probes each file with different offsets and sizes.
    #[test]
    fn a_sweep_of_different_spans_is_never_served() {
        let intent = ReadIntent::default();
        let now = Instant::now();
        for ino in 0..10u64 {
            intent.classify_at(1, true, ino, 0, 131_072, now);
        }
        // The shape observed from the file manager in the wild: head at two sizes, then a tail
        // read, then the head again at yet another size - never the same span twice.
        let probes = [
            (0u64, 16_384u32),
            (0, 4_096),
            (4_284_416, 4_096),
            (0, 8_192),
        ];
        for (round, &(offset, size)) in probes.iter().enumerate() {
            assert_eq!(
                intent.classify_at(1, true, 4242, offset, size, now),
                ReadVerdict::Refuse,
                "probe #{round} of a swept file was served, so the original would be downloaded"
            );
        }
    }

    /// Opening a single preview must work: one or two files is not a sweep.
    #[test]
    fn opening_a_few_files_is_served() {
        let intent = ReadIntent::default();
        let now = Instant::now();
        for ino in 0..SWEEP_DISTINCT_FILES as u64 - 1 {
            for _ in 0..10 {
                assert_eq!(
                    intent.classify_at(1, true, ino, 0, PROBE_MAX, now),
                    ReadVerdict::Allow,
                    "opening file {ino} was refused before a sweep was established"
                );
            }
        }
    }

    /// Read size is the signal: a probe-sized sample is refused, a content-sized read is not.
    #[test]
    fn probe_reads_are_refused_and_content_reads_are_not() {
        for size in [4_096u32, 8_192, 16_384, PROBE_MAX] {
            let intent = ReadIntent::default();
            let now = Instant::now();
            for ino in 0..10u64 {
                intent.classify_at(1, true, ino, 0, PROBE_MAX, now);
            }
            assert_eq!(
                intent.classify_at(1, true, 999, 0, size, now),
                ReadVerdict::Refuse,
                "a mid-sweep probe of {size} bytes was not refused"
            );
        }
        for size in [PROBE_MAX + 1, 131_072, 1024 * 1024] {
            let intent = ReadIntent::default();
            let now = Instant::now();
            for ino in 0..10u64 {
                intent.classify_at(1, true, ino, 0, PROBE_MAX, now);
            }
            assert_eq!(
                intent.classify_at(1, true, 999, 0, size, now),
                ReadVerdict::Allow,
                "a mid-sweep content read of {size} bytes was refused, so a copy would fail"
            );
        }
    }

    /// Processes must be classified independently.
    #[test]
    fn processes_are_tracked_separately() {
        let intent = ReadIntent::default();
        let now = Instant::now();
        for ino in 0..20u64 {
            intent.classify_at(1, true, ino, 0, PROBE_MAX, now);
        }
        // A second file manager has its own history, so a single open is still ordinary use.
        assert_eq!(
            intent.classify_at(2, true, 500, 0, PROBE_MAX, now),
            ReadVerdict::Allow,
            "one process inherited another's sweep history"
        );
    }

    /// A stale run must not let a later sample through as if it continued anything.
    ///
    /// Tested on the probe directly, because `classify` cannot express it: [`RUN_WINDOW`] is
    /// longer than [`SWEEP_WINDOW`], so by the time a run has gone stale the sweep evidence that
    /// made the process subject to the guard has expired as well, and the read is served as
    /// ordinary use rather than as a continuing read.
    #[test]
    fn run_evidence_expires() {
        let start = Instant::now();
        let probe = FileProbe {
            last_end: 131_072,
            last_at: Some(start),
        };
        assert!(
            probe.continues(131_072, start),
            "an immediately continuing read was not recognised"
        );
        assert!(
            probe.continues(131_072, start + RUN_WINDOW),
            "a read at the edge of the window was not recognised"
        );
        let stale = start + RUN_WINDOW + Duration::from_millis(1);
        assert!(
            !probe.continues(131_072, stale),
            "a run that ended long ago still counted as continuing"
        );
        assert!(
            !probe.continues(262_144, start),
            "a forward jump was treated as continuing the run"
        );
        assert!(
            !probe.continues(0, start),
            "a restart from zero was treated as continuing the run"
        );
    }

    /// Evidence of a sweep must expire, so browsing and then copying in one process works.
    #[test]
    fn sweep_evidence_expires() {
        let intent = ReadIntent::default();
        let start = Instant::now();
        for ino in 0..20u64 {
            for _ in 0..4 {
                intent.classify_at(1, true, ino, 0, PROBE_MAX, start);
            }
        }
        let later = start + SWEEP_WINDOW + Duration::from_millis(1);
        assert_eq!(
            intent.classify_at(1, true, 999, 0, PROBE_MAX, later),
            ReadVerdict::Allow,
            "stale sweep evidence still caused a refusal"
        );
    }
}
