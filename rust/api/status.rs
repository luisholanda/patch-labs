//! # Standard gRPC status-based error handling
//!
//! This crate provides a [`Status`] type that can be used to gracefully
//! handle errors across service boundaries. The API is heavily focused
//! in gRPC and `google.rpc` capable services.
//!
//! A [`Status`] instance can optionally include a series of payload [key, value]
//! pairs, the key being a URL that maps to a specific protobuf type, which
//! is encoded in the value. These payloads can be used to provide semantic
//! and contextual information about a specific error.
use std::error::Error;

use tonic::Code;
use tonic_types::{ErrorDetails, StatusExt};

/// A type that can be a `T` or [`Status`].
pub type StatusOr<T> = Result<T, Status>;

/// A standard gRPC status. Incremented with a [`google.rpc.Status`].
///
/// This is the default error type used through out the system. The
/// API is inspired by C++ [`absl::Status`].
///
/// [`google.rpc.status`]: https://github.com/googleapis/googleapis/blob/c3ec9bc89a6f46e0ce91b04eec4ea3fb1ad2a748/google/rpc/status.proto#L28
/// [`absl::Status`]: https://github.com/abseil/abseil-cpp/blob/ce1d3484756e20ce96f6404eb62362c87fbd584a/absl/status/status.h#L338
#[must_use]
#[derive(Debug)]
pub struct Status(Box<Inner>);

#[derive(Debug)]
struct Inner {
    details: ErrorDetails,
    code: Code,
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl Error for Status {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        let err = self.0.source.as_deref()?;

        Some(err)
    }
}

impl From<Box<dyn Error + Send + Sync>> for Status {
    fn from(err: Box<dyn Error + Send + Sync>) -> Self {
        let mut s = Self::internal(err.to_string());
        s.0.source.replace(err);

        s
    }
}

impl From<Status> for tonic::Status {
    fn from(s: Status) -> Self {
        Self::with_error_details(s.code(), s.0.message, s.0.details)
    }
}

impl Status {
    fn new(code: Code, message: String) -> Self {
        Self(Box::new(Inner {
            details: ErrorDetails::new(),
            code,
            message,
            source: None,
        }))
    }

    /// Get the code of this status.
    pub fn code(&self) -> Code {
        self.0.code
    }

    /// Get the message of this status.
    pub fn message(&self) -> &str {
        &self.0.message
    }

    /// Get the details of this status.
    pub fn details(&self) -> &ErrorDetails {
        &self.0.details
    }

    /// Get the details of this status.
    pub fn details_mut(&mut self) -> &mut ErrorDetails {
        &mut self.0.details
    }
}

macro_rules! gen_constructors {
    ($($(#[$meta: meta])* $name: ident => $code: ident,)+) => {
        impl Status {
        $(
            $(#[$meta])*
            pub fn $name(message: impl std::fmt::Display) -> Self {
                Self::new(Code::$code, message.to_string())
            }
        )+
        }
    };
}

gen_constructors! {
    /// The operation was cancelled
    cancelled => Cancelled,

    /// Unknown error.
    unknown => Unknown,

    /// Client specified an invalid argument.
    ///
    /// Note that this differs from `FailedPrecondition`. `InvalidArgument`
    /// indicates arguments that are problematic regardless of the state of the system.
    invalid_argument => InvalidArgument,

    /// Deadline expired before operation coud complete.
    ///
    /// For operations that change the state of the system, this error may
    /// be returned even if the operation has completed successfully. For
    /// example, a successful response from a server could have been delayed
    /// long enough for the deadline to expire.
    deadline_exceeded => DeadlineExceeded,

    /// Some requested entity was not found.
    not_found => NotFound,

    /// Some entity that we attempted to create already exists.
    already_exists => AlreadyExists,

    /// The caller does not have permission to execute the specified operation.
    ///
    /// This should not be used for rejections caused by exhauting some resource, use
    /// `ResoureceExhausted` instead. It should also not be used if we cannot identify
    /// the called, use `Unauthenticated` in this case.
    permission_denied => PermissionDenied,

    /// Some resource has been exhausted, perhaps a per-user quota, or perhaps
    /// the file system is out of space.
    resource_exhausted => ResourceExhausted,

    /// Operation was rejected because the system is not in a state required
    /// for the operation's execution. For example, directory to be deleted
    /// may be non-empty.
    ///
    /// A litmus test that may help a service implementor in deciding between
    /// `FailedPrecondition`, `Aborted` and `Unavailable`: (a) use `Unavailable`
    /// is the client can retry just the failing call, (b) use `Aborted` if the
    /// client should retry at a higher-level (e.g. restartina read-modify-write
    /// sequence), (c) use `FailedPrecondition` if the client should not retry
    /// until the system state has been explicitly fixed. E.g., if an "rmdir"
    /// fails because the direcory is non-empty, `FailedPrecondition` should be
    /// returned since the client should not retry unless they have first fixed
    /// up the directory by deleting files from it.
    failed_precondition => FailedPrecondition,

    /// The operation was aborted, typically due to a concurrency issue like
    /// sequecner check failures, transaction aborts, etc.
    aborted => Aborted,

    /// Operation was attempted past the valid range. E.g., seeking or reading
    /// past end of file.
    ///
    /// Unlike `InvalidArgument`, this error indicates a problem that may be
    /// fixed if the system state changes. For example, a 32-bit file system
    /// will generate `InvalidArgument` if asked to read at an offset that is
    /// not in the range `[0, 2³²-1]`, but it will generate `OutOfRange` if
    /// asked to read from an offset past the current file size.
    ///
    /// There is a fair bit of overlap between `FailedPrecondition` and `OutOfRange`.
    /// We recommend using `OutOfRange` (the more specific error) when it applies
    /// so that callers who are iterating through a space can easily look for an
    /// `OutOfRange` error to detect when they are done.
    out_of_range => OutOfRange,

    /// Operation is not implemented or not supported/enabled in this service.
    unimplemented => Unimplemented,

    /// Internal errors. Means some invariants expected by underlying system has
    /// been broken.
    ///
    /// If you see one of these erros, something is very broken.
    internal => Internal,

    /// The service is currently unavailable. This is most likely a transient condition
    /// and may be corrected by retrying with a back-off.
    unavailable => Unavailable,

    /// Unrecoverable data loss or corruption.
    data_loss => DataLoss,

    /// The request does not have valid authentication credentials for the operation.
    unauthenticated => Unauthenticated,
}
