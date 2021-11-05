//! Disk Scheme
//!
//! An abstract directory structure, used to describe and construct a POSIX-like
//! filesystem.
//!
//! Some notable features include:
//! * Pattern matching
//! * Variable substitution
//! * Reusable definitions (sub-schemas)
//! * Symlinked directory construction
//!
//! The abstract directory structure is defined and manipulated by the [`schema`] module,
//! which also provides a means to load schemas from disk.
//!
//! Construction of physical disk entries is handled by the [`application`] module.

pub mod apply;
pub mod context;
pub mod install;
pub mod schema;
