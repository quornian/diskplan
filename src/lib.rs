//! A system for specifying abstract directory trees and applying them to disk.
//!
//! # Schema Tree
//!
//! Diskplan uses a simple language to define a tree of files, directories and symlinks.
//! Here is the skeleton of an example schema tree:
//! ```
//! let schema_root = diskplan::schema::parse_schema(
//! # concat!(
//! "
//! top_level_directory/
//! # " /*
//!     ...
//! # */, "
//!     file
//! #         #source /src/example
//! # " /*
//!         ...
//! # */, "
//!     sub_directory/
//! # " /*
//!         ...
//! # */, "
//!         symlink_file -> ...
//! #             #source /src/example
//! # " /*
//!             ...
//! # */, "
//!         symlink_directory/ -> ...
//! # " /*
//!             ...
//! # */, "
//! "
//! # )
//! )?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//! Directory entries are named, sub-directories are signfied by a slash, symlinks by an arrow.
//!
//! Tags (prefixed by a `#`) are used to set properties of a node. Variables are used to
//!
//! Further to the above skeleton:
//! * Files must specify a `#source` with a local path to the file's content
//! * Properties of any level can be set using `#owner`, `#group` and `#mode` tags
//! * Entries in the tree can be given names with `#def` and reused elsewhere with `#use`
//! * Variables can be set with `#let` and used in path expressions
//! * Entries can be dynamic (e.g. `$somename`), with `#match` used to set the pattern
//!
//! For full details, see the [`schema`] module. For now, here is a more complete example
//! using all of the above features:
//!
//! ```
//! let schema_root = diskplan::schema::parse_schema(
//! "
#![doc = include_str!("doc/fragments/schema")]
//! "
//! )?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! Given this existing structure in the filesystem:
//! ```
//! let input_tree = "
#![doc = include_str!("doc/fragments/input_tree")]
//! ";
//! ```
//!
//! This output structure is produced:
//! ```
//! let output_tree = "
#![doc = include_str!("doc/fragments/output_tree")]
//! ";
//! #
//! # // Now verify it so our docs are always correct
//! # let target = "/local";
//! # use diskplan::{doctests::verify_trees, schema::parse_schema};
//! # let schema_root = parse_schema(include_str!("doc/fragments/schema"))?;
//! # let input_tree = include_str!("doc/fragments/input_tree");
//! # verify_trees(&schema_root, input_tree, output_tree, target)?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Traversal
//!
//! To look at how a schema tree would apply to a directory on disk, we need a
//! [`Filesystem`][crate::filesystem::Filesystem] to apply it to, and a starting path.
//!
//! We can use the in-memory filesystem to test:
//! ```
//! use diskplan::{
//!     filesystem::{Filesystem, MemoryFilesystem},
//!     traversal::traverse,
//!     schema::parse_schema
//! };
//!
//! // Construct a schema
//! let schema_root = parse_schema("
//! directory/
//!     #mode 777
//! ")?;
//!
//! // Define the target location to apply it
//! let target = "/local";
//!
//! // Construct the initial filesystem
//! let fs = MemoryFilesystem::new();
//!
//! // Run the traversal to apply the tree to the filesystem
//! fs.create_directory(target);
//! traverse(&schema_root, &fs, target)?;
//!
//! assert!(fs.is_directory("/local/directory"));
//! #
//! # Ok::<(), anyhow::Error>(())
//! ```
//!

pub mod filesystem;
pub mod schema;
pub mod traversal;

//#[cfg(test)]
#[doc(hidden)]
pub mod doctests;
