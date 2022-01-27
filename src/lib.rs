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
//! #         :source /src/example
//! # " /*
//!         ...
//! # */, "
//!     sub_directory/
//! # " /*
//!         ...
//! # */, "
//!         symlink_file -> ...
//! #             :source /src/example
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
//! Tags (prefixed by a `:`) are used to set properties of a node. Variables are used to
//!
//! Further to the above skeleton:
//! * Files must specify a `:source` with a local path to the file's content
//! * Properties of any level can be set using `:owner`, `:group` and `:mode` tags
//! * Entries in the tree can be given names with `:def` and reused elsewhere with `:use`
//! * Variables can be set with `:let` and used in path expressions
//! * Entries can be dynamic (e.g. `$somename`), with `:match` used to set the pattern
//!
//! For full details, see the [`schema`] module. For now, here is a more complete example
//! using all of the above features:
//!
//! ```
//! let schema_root = diskplan::schema::parse_schema(
//! "
//! :let remote_disk = /net/remote
//!
//! :mode 777
//!
//! :def admin_directory/
//!     :owner root
//!     :group root
//!     :mode 750
//!
//! $zone/
//!     :match zone_[a-z]
//!
//!     description.md
//!         :source ${remote_disk}/resources/common_zone_description.md
//!
//!     admin/
//!         :use admin_directory
//!
//!         zone_image.img
//!             :source ${remote_disk}/resources/${zone}.img
//!
//!         storage/ -> ${remote_disk}/storage_pool/${zone}
//!             database.db
//!                 :source ${remote_disk}/resources/empty_database.db
//!
//! "
//! )?;
//!
//! // Given this existing structure in the filesystem:
//! let input_tree = "
//! /
//! ├── net/
//! │   └── remote/
//! │       └── resources/
//! │           ├── common_zone_description.md
//! │           ├── empty_database.db
//! │           ├── zone_a.img
//! │           └── zone_b.img
//! └── local/
//!     ├── non_zone/
//!     ├── zone_a/
//!     └── zone_b/
//! ";
//!
//! // This output structure is produced:
//! let output_tree = "
//! /
//! ├── net/
//! │   └── remote/
//! │       ├── resources/
//! │       │   ├── common_zone_description.md
//! │       │   ├── empty_database.db
//! │       │   ├── zone_a.img
//! │       │   └── zone_b.img
//! │       └── storage_pool/
//! │           ├── zone_a/
//! │           │   └── database.db
//! │           └── zone_b/
//! │               └── database.db
//! └── local/
//!     ├── non_zone/
//!     ├── zone_a/
//!     │   ├── admin/
//!     │   │   ├── storage -> /net/remote/storage_pool/zone_a
//!     │   │   └── zone_image.img
//!     │   └── description.md
//!     └── zone_b/
//!         ├── admin/
//!         │   ├── storage -> /net/remote/storage_pool/zone_b
//!         │   └── zone_image.img
//!         └── description.md
//! ";
//! #
//! # // Now verify it so our docs are always correct
//! # mod doctests { include!{"doctests.rs"} }
//! # doctests::verify_trees(&schema_root, input_tree, output_tree, "/local")?;
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
//!     filesystem::{Filesystem, MemoryFilesystem, SetAttrs},
//!     traversal::traverse,
//!     schema::parse_schema
//! };
//! # mod doctests { include!{"doctests.rs"} }
//!
//! // Construct a schema
//! let schema_root = parse_schema("
//! directory/
//!     :mode 777
//! ")?;
//!
//! // Define the target location to apply it
//! let target = "/local";
//!
//! // Construct the initial filesystem
//! let mut fs = MemoryFilesystem::new();
//!
//! // Run the traversal to apply the tree to the filesystem
//! fs.create_directory(target, SetAttrs::default());
//! traverse(&schema_root, &mut fs, target)?;
//!
//! assert!(fs.is_directory("/local/directory"));
//! #
//! # Ok::<(), anyhow::Error>(())
//! ```
//!

pub mod filesystem;
pub mod schema;
pub mod traversal;

#[cfg(test)]
mod tests;
