# nextcloud-sdk-request

> Pure state machine for per-request retry scheduling and exponential backoff.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-sdk-request = "0.1.2"
```

## Features

- Deterministic, runtime-independent retry state machine.
- Configurable retry budget, exponential backoff with jitter, and retryable status codes.
- Tracks per-request attempt metrics for progress bars and telemetry.

## Example

```rust,no_run
use nextcloud_sdk_request::{Effect, RequestMachine, RetryPolicy};

let mut machine = RequestMachine::new();
let policy = RetryPolicy::default();

let effect = machine.step(&policy);
assert_eq!(effect, Effect::Send);
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
