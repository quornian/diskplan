//! A system for specifying abstract directory trees and applying them to disk.
//!
//! # Schemas
//!
//! Diskplan uses a simple language to define its tree of filesystem nodes.
//! Here is the skeleton of an example [`schema`]:
//! ```
//! let schema = diskplan::schema::parse_schema(
//! # concat!(
//! "
//! top_level_directory/
//! # " /*
//!     ...
//! # */, "
//!     file
//! #         #source example
//! # " /*
//!         ...
//! # */, "
//!     sub_directory/
//! # " /*
//!         ...
//! # */, "
//!         symlink_file -> ...
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
//! Directory entries are named, with sub-directories signfied by a slash, symlinks with an arrow.
//!
//! Further to the above skeleton example:
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
//! let schema = diskplan::schema::parse_schema(
//! "
//! #let remote_disk = /net/remote
//!
//! #mode 777
//!
//! #def admin_directory/
//!     #owner admin
//!     #group admin
//!     #mode 750
//!
//! $zone/
//!     #match zone_[a-z]
//!
//!     description.md
//!         #source resources/common_zone_description.md
//!
//!     admin/
//!         #use admin_directory
//!
//!         zone_image.img
//!             #source ${remote_disk}/resources/${zone}.img
//!
//!         storage/ -> ${remote_disk}/storage_pool/${zone}/
//!             database.db
//!                 #source resources/empty_database.db
//!
//! "
//! )?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Application
//!
//! To look at how a schema would apply to a directory on disk, we first associate the two with a
//! [`Context`][context::Context] object, and then run [`gather_actions`][apply::gather_actions]:
//! ```
//! use std::path::Path;
//! use diskplan::{schema::parse_schema, context::Context, apply::gather_actions};
//!
//! // Construct a Schema
//! let schema = parse_schema("
//! directory/
//!     #mode 777
//! ")?;
//!
//! // Define the target location to apply it
//! let target = Path::new("/tmp/root");
//!
//! // Build the initial, root level Context
//! let context = Context::new(&schema, target, Path::new("."));
//!
//! // Collect the actions to be performed, and print them out
//! let actions = gather_actions(&context)?;
//! for action in actions {
//!     println!("Would apply: {:#?}", action);
//! }
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! ```text
//! Would apply: CreateDirectory {
//!     path: "/tmp/root",
//!     meta: Meta {
//!         owner: None,
//!         group: None,
//!         mode: None,
//!     },
//! }
//! Would apply: CreateDirectory {
//!     path: "/tmp/root/directory",
//!     meta: Meta {
//!         owner: None,
//!         group: None,
//!         mode: Some(
//!             511,
//!         ),
//!     },
//! }
//! ```
//!
//! See the [`install`] module for how to apply actions to the filesystem.
//!

// pub mod apply;
// pub mod context;
// pub mod install;
pub mod schema;
