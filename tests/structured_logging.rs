//! Verifies that the client layers emit structured logs on all five levels.
//!
//! `tracing` caches the interest of every callsite process-wide, and the first
//! thread to touch a callsite decides that cached value: a thread without a
//! subscriber resolves it to `Interest::never`, which then suppresses the event
//! for every other thread. This test therefore has to be the only test in its
//! binary — a second test running in parallel would be free to poison the
//! callsites this one observes. Keep this file limited to a single `#[test]`;
//! further logging assertions belong in their own integration test file.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use nexia_nextcloud_sdk::{BandwidthLimit, ClientBuilder, SecretString, ServerUrl, StoreKey};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

#[derive(Clone, Default)]
struct LevelTracker {
    trace_count: Arc<AtomicUsize>,
    debug_count: Arc<AtomicUsize>,
    info_count: Arc<AtomicUsize>,
    warn_count: Arc<AtomicUsize>,
    error_count: Arc<AtomicUsize>,
}

impl LevelTracker {
    fn count(&self) -> [usize; 5] {
        [
            self.trace_count.load(Ordering::SeqCst),
            self.debug_count.load(Ordering::SeqCst),
            self.info_count.load(Ordering::SeqCst),
            self.warn_count.load(Ordering::SeqCst),
            self.error_count.load(Ordering::SeqCst),
        ]
    }
}

impl Subscriber for LevelTracker {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let counter = match *event.metadata().level() {
            tracing::Level::TRACE => &self.trace_count,
            tracing::Level::DEBUG => &self.debug_count,
            tracing::Level::INFO => &self.info_count,
            tracing::Level::WARN => &self.warn_count,
            tracing::Level::ERROR => &self.error_count,
        };
        counter.fetch_add(1, Ordering::SeqCst);
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "structured-logging-{name}-{}-{:?}.db",
        std::process::id(),
        std::thread::current().id()
    ))
}

#[tokio::test]
async fn client_emits_all_log_levels() {
    let tracker = LevelTracker::default();

    {
        let _guard = tracing::subscriber::set_default(tracker.clone());

        tracing::trace!("trace event");
        tracing::debug!("debug event");
        tracing::info!("info event");
        tracing::warn!("warn event");
        tracing::error!("error event");

        let server_url = ServerUrl::parse("https://cloud.example.org").expect("server url");
        let auth = temp_path("auth");
        let cache = temp_path("cache");

        let client = ClientBuilder::new()
            .server_url(server_url)
            .storage_paths(&auth, &cache)
            .store_key(StoreKey::generate().expect("store key"))
            .initial_credentials("alice", SecretString::new("secret"))
            .build()
            .await
            .expect("build client");

        client.set_upload_limit(BandwidthLimit::from_kib(500));
        client.set_download_limit(BandwidthLimit::from_mib(5));

        std::fs::remove_file(&auth).ok();
        std::fs::remove_file(&cache).ok();
    }

    let [trace, debug, info, warn, error] = tracker.count();
    assert!(trace >= 1, "expected a trace event, saw {trace}");
    assert!(debug >= 3, "expected at least 3 debug events, saw {debug}");
    assert!(info >= 3, "expected at least 3 info events, saw {info}");
    assert!(warn >= 1, "expected a warn event, saw {warn}");
    assert!(error >= 1, "expected an error event, saw {error}");
}
