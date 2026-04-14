//! This crate provides the [`MaybeMut`] that abstracts away smart pointers,
//! locks etc as if they did not exist.
//!
//! This is usually not desirable, because it can easily lead to
//! deadlocks and really bad performance. But sometimes, you simply do not care
//! (for example in simple UI applications)

mod maybe_mut;
pub use maybe_mut::*;
