//! The account lifecycle: from an unknown server to a usable session.
//!
//! The machine follows Nextcloud's Login Flow v2, which exists so a client
//! never handles the user's password: the application opens a login URL in
//! the user's browser, the user authenticates and grants access there, and the
//! client polls until the server hands back an app password.
//!
//! ```text
//!                StartRequested          FlowStarted
//!    Idle ──────────────────────► Idle ──────────────► AwaitingLogin
//!                                                          │  ▲
//!                                        PollPending (404) │  │ PollPending
//!                                                          ▼  │
//!                                                    AwaitingGrant
//!                                                          │
//!                                            Granted      │
//!                                                          ▼
//!                     CredentialsRejected        ┌── Authenticated
//!                          ◄──────────────────────┘
//!                          ▼
//!                   ReauthRequired ──► Idle (LogoutRequested)
//! ```
//!
//! The machine is pure: [`SessionMachine::handle`] returns the
//! [`Command`] the application has to carry out and never performs I/O
//! itself. Events that cannot apply in the current state are reported as a
//! [`SessionError`] instead of being applied, so a poll answer that arrives
//! after a logout cannot resurrect a dead session.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod command;
mod error;
mod event;
mod machine;
mod state;

pub use command::{Command, PollTarget};
pub use error::SessionError;
pub use event::{LoginStart, LoginSuccess, SessionEvent};
pub use machine::SessionMachine;
pub use state::{Account, LoginFlow, PollToken, SessionState};
