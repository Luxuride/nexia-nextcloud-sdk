use std::fmt;
use std::num::NonZeroU64;

/// Bandwidth rate limit constraint.
///
/// Expresses either an unconstrained rate ([`BandwidthLimit::Unlimited`]) or an
/// upper bound in bytes per second ([`BandwidthLimit::BytesPerSecond`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BandwidthLimit {
    /// No bandwidth restriction.
    #[default]
    Unlimited,
    /// Maximum bandwidth allocation in bytes per second.
    BytesPerSecond(NonZeroU64),
}

impl BandwidthLimit {
    /// Creates an unlimited bandwidth rate constraint.
    pub const fn unlimited() -> Self {
        Self::Unlimited
    }

    /// Creates a rate limit with the specified bytes per second.
    ///
    /// If `rate` is zero, this returns [`BandwidthLimit::Unlimited`].
    pub const fn bytes_per_second(rate: u64) -> Self {
        match NonZeroU64::new(rate) {
            Some(nz) => Self::BytesPerSecond(nz),
            None => Self::Unlimited,
        }
    }

    /// Creates a rate limit in kibibytes per second (1 KiB = 1,024 bytes).
    ///
    /// If `kib` is zero, returns [`BandwidthLimit::Unlimited`].
    pub const fn from_kib(kib: u64) -> Self {
        Self::bytes_per_second(kib.saturating_mul(1024))
    }

    /// Creates a rate limit in mebibytes per second (1 MiB = 1,048,576 bytes).
    ///
    /// If `mib` is zero, returns [`BandwidthLimit::Unlimited`].
    pub const fn from_mib(mib: u64) -> Self {
        Self::bytes_per_second(mib.saturating_mul(1024 * 1024))
    }

    /// Whether this limit is unconstrained.
    pub const fn is_unlimited(&self) -> bool {
        matches!(self, Self::Unlimited)
    }

    /// The maximum bytes permitted per second, or `None` if unlimited.
    pub const fn as_bytes_per_second(&self) -> Option<NonZeroU64> {
        match self {
            Self::Unlimited => None,
            Self::BytesPerSecond(rate) => Some(*rate),
        }
    }
}

impl fmt::Display for BandwidthLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unlimited => write!(f, "unlimited"),
            Self::BytesPerSecond(rate) => {
                let bytes = rate.get();
                if bytes >= 1024 * 1024 && bytes % (1024 * 1024) == 0 {
                    write!(f, "{} MiB/s", bytes / (1024 * 1024))
                } else if bytes >= 1024 && bytes % 1024 == 0 {
                    write!(f, "{} KiB/s", bytes / 1024)
                } else {
                    write!(f, "{bytes} B/s")
                }
            }
        }
    }
}

impl From<u64> for BandwidthLimit {
    fn from(bytes_per_sec: u64) -> Self {
        Self::bytes_per_second(bytes_per_sec)
    }
}

impl From<Option<u64>> for BandwidthLimit {
    fn from(opt: Option<u64>) -> Self {
        match opt {
            Some(rate) => Self::bytes_per_second(rate),
            None => Self::Unlimited,
        }
    }
}
