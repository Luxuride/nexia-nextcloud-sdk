//! Asynchronous download queue scheduling, live progress tracking, and event streaming.

pub mod events;
pub mod executor;
pub mod priority;
pub mod progress;
pub mod queue;
pub mod snapshot;
pub mod status;
pub mod task;
pub mod task_id;

pub use events::DownloadQueueEvent;
pub use executor::DownloadExecutor;
pub use priority::DownloadPriority;
pub use progress::{DownloadProgress, format_binary_bytes};
pub use queue::SyncDownloadQueue;
pub use snapshot::DownloadQueueSnapshot;
pub use status::DownloadStatus;
pub use task::DownloadTask;
pub use task_id::DownloadTaskId;
