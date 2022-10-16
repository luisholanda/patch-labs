//! # Database system.
//!
//! This crate provides a generic interface for a key-value
//! database store. The interface is generic over the underlying
//! storage, meaning that application code can be easily tested
//! via in-memory databases.
use bytes::BytesMut;
use prost::Message;

pub(crate) mod counter;
pub(crate) mod range;
pub(crate) mod storage;

pub use self::{counter::Counter, range::RangeQuery, storage::*};

/// A database entity.
///
/// Entities represents some data of interest to the system.
pub trait Entity: Sized + Message {
    /// Write the entity to the given buffer.
    fn write(&self, out: &mut BytesMut);

    /// Read an entity value from the given buffer.
    ///
    /// # Panics
    ///
    /// This function is allowed to panic if the buffer contains
    /// invalid data.
    ///
    /// Note that it is expected that entities evolve in a backwards
    /// compatible way.
    fn read(bytes: &[u8]) -> Self;
}
