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
//! The abstract directory structure is defined and manipulated by the [`definition`] module,
//! which also provides a means to load schemas from disk.
//!
//! Construction of physical disk entries is handled by the [`application`] module.

/// Definition of a schema and means for creating them
///
/// The [fromdisk] sub-module specifies a disk-based Schema and the mechanism for loading it
///
pub mod definition;

/// Application of a schema to construct filesystem entities on disk
///
pub mod application;
