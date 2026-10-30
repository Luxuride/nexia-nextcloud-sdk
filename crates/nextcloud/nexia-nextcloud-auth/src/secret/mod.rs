//! Plaintext handling that keeps secrets out of logs and `Debug` output.

mod secret_string;

pub use secret_string::SecretString;
