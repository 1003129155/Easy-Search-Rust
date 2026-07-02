// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Process-lifetime policy for the EasySearch daemon.

/// Reason the daemon should stop accepting new work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShutdownReason {
    /// An explicit shutdown request was received.
    Requested,
}
