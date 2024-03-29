//! Configuration for the system
//!
//! Example config file:
//! ```
//! # use diskplan_config::ConfigFile;
//! # let config_text = r#"
#![doc = include_str!("../examples/quickstart/diskplan.toml")]
//! # "#;
//! # let config: ConfigFile = config_text.try_into().unwrap();
//! # let stem = config.stems.get("main").expect("no main stem");
//! # assert_eq!(stem.root().path().as_str(), "/tmp/diskplan-root");
//! # assert_eq!(stem.schema().as_str(), "simple-schema.diskplan");
//! ```
#![warn(missing_docs)]

use std::{collections::HashMap, fmt::Write as _, ops::Deref};

use anyhow::{anyhow, Context as _, Result};
use camino::{Utf8Path, Utf8PathBuf};

use diskplan_filesystem::Root;
use diskplan_schema::SchemaNode;

mod cache;
mod file;
pub use self::{
    cache::SchemaCache,
    file::{ConfigFile, ConfigStem},
};

/// Application configuration
pub struct Config<'t> {
    /// The directory to produce. This must be absolute and begin with one of the configured roots
    target: Utf8PathBuf,

    /// Whether to apply the changes (otherwise, only simulate and print)
    apply: bool,

    /// Directory to search for schemas
    schema_directory: Utf8PathBuf,

    /// Map user names, for example "root:admin,janine:jfu"
    usermap: HashMap<String, String>,

    /// Map groups names
    groupmap: HashMap<String, String>,

    stems: Stems<'t>,
}

impl<'t> Config<'t> {
    /// Constructs a new application configuration
    ///
    /// The `target` path defines a directory into which we will begin
    ///
    /// The `apply` flag controls whether changes should be applied to
    /// the filesystem or just reported
    pub fn new(target: impl AsRef<Utf8Path>, apply: bool) -> Self {
        Config {
            target: target.as_ref().to_owned(),
            apply,
            schema_directory: Utf8PathBuf::from("/"),
            usermap: Default::default(),
            groupmap: Default::default(),
            stems: Default::default(),
        }
    }

    /// Loads configuation options from the given `path`
    pub fn load(&mut self, path: impl AsRef<Utf8Path>) -> Result<()> {
        let ConfigFile {
            stems,
            schema_directory,
        } = ConfigFile::load(path.as_ref())?;
        self.schema_directory = schema_directory.unwrap_or_else(|| {
            path.as_ref()
                .parent()
                .expect("No parent directory for config file")
                .to_owned()
        });
        for (_, stem) in stems.into_iter() {
            let schema_path = self.schema_directory.join(stem.schema());
            self.stems.add(stem.root().to_owned(), schema_path)
        }
        Ok(())
    }

    /// Updates this configuration's user name map with the one provided
    pub fn apply_user_map(&mut self, usermap: HashMap<String, String>) {
        self.usermap.extend(usermap.into_iter())
    }

    /// Updates this configuration's group name map with the one provided
    pub fn apply_group_map(&mut self, groupmap: HashMap<String, String>) {
        self.groupmap.extend(groupmap.into_iter())
    }

    /// The path intended to be constructed
    pub fn target_path(&self) -> &Utf8Path {
        self.target.as_ref()
    }

    /// Whether to apply the changes to disk
    pub fn will_apply(&self) -> bool {
        self.apply
    }

    /// Add a root and schema definition file path pair
    pub fn add_stem(&mut self, root: Root, schema_path: impl AsRef<Utf8Path>) {
        self.stems.add(root, schema_path)
    }

    /// Add a root and schema definition file path pair, adding its already parsed schema to the cache
    ///
    /// The file path will not be read; this can be used for testing
    pub fn add_precached_stem(
        &mut self,
        root: Root,
        schema_path: impl AsRef<Utf8Path>,
        schema: SchemaNode<'t>,
    ) {
        self.stems.add_precached(root, schema_path, schema)
    }

    /// Returns an iterator over the configured [`Root`]s
    pub fn stem_roots(&self) -> impl Iterator<Item = &Root> {
        self.stems.roots()
    }

    /// Returns the schema for a given path, loaded on demand, or an error if the schema cannot be
    /// found, has a syntax error, or otherwise fails to load
    pub fn schema_for<'s, 'p>(&'s self, path: &'p Utf8Path) -> Result<(&SchemaNode<'t>, &Root)>
    where
        's: 't,
    {
        self.stems.schema_for(path)
    }

    /// Applies the user map to the given user name, returning itself if no mapping exists for
    /// this name
    pub fn map_user<'a>(&'a self, name: &'a str) -> &'a str {
        self.usermap.get(name).map(|s| s.deref()).unwrap_or(name)
    }

    /// Applies the group map to the given group name, returning itself if no mapping exists for
    /// this name
    pub fn map_group<'a>(&'a self, name: &'a str) -> &'a str {
        self.groupmap.get(name).map(|s| s.deref()).unwrap_or(name)
    }
}

/// Collection of rooted schemas; a map of each [`Root`] to the [`SchemaNode`] configured for this root
#[derive(Default)]
pub struct Stems<'t> {
    /// Maps root path to the schema definition's file path
    path_map: HashMap<Root, Utf8PathBuf>,

    /// A cache of loaded schemas from their definition files
    cache: SchemaCache<'t>,
}

impl<'t> Stems<'t> {
    /// Constructs an empty mapping
    pub fn new() -> Self {
        Default::default()
    }

    /// Configures the given `root` path with the path where a schema for this root may be found
    pub fn add(&mut self, root: Root, schema_path: impl AsRef<Utf8Path>) {
        self.path_map.insert(root, schema_path.as_ref().to_owned());
    }

    /// Configures the given `root` path with the path where a schema for this root may be found
    /// but then populates the internal cache with the schema data itself, avoiding any disk access
    ///
    /// This is primarily used for tests
    pub fn add_precached(
        &mut self,
        root: Root,
        schema_path: impl AsRef<Utf8Path>,
        schema: SchemaNode<'t>,
    ) {
        let schema_path = schema_path.as_ref();
        self.cache.inject(schema_path, schema);
        self.add(root, schema_path);
    }

    /// Returns an iterator over the roots configures in this map
    pub fn roots(&self) -> impl Iterator<Item = &Root> {
        self.path_map.keys()
    }

    /// Looks up the schema associated with the root of a given `path` within this root
    pub fn schema_for<'s, 'p>(&'s self, path: &'p Utf8Path) -> Result<(&SchemaNode<'t>, &Root)>
    where
        's: 't,
    {
        let mut longest_candidate = None;
        for (root, schema_path) in self.path_map.iter() {
            if path.starts_with(root.path()) {
                match longest_candidate {
                    None => longest_candidate = Some((root, schema_path)),
                    Some(prev) => {
                        if root.path().as_str().len() > prev.0.path().as_str().len() {
                            longest_candidate = Some((root, schema_path))
                        }
                    }
                }
            }
        }

        if let Some((root, schema_path)) = longest_candidate {
            tracing::trace!(
                r#"Schema for path "{}", found root "{}", schema "{}""#,
                path,
                root.path(),
                schema_path
            );
            let schema = self.cache.load(schema_path).with_context(|| {
                format!(
                    "Failed to load schema {} for configured root {} (for target path {})",
                    schema_path,
                    root.path(),
                    path
                )
            })?;
            Ok((schema, root))
        } else {
            let mut roots = String::new();
            for root in self.roots() {
                write!(roots, "\n - {}", root.path())?;
            }
            Err(anyhow!(
                "No root/schema for path {}\nConfigured roots:{}",
                path,
                roots
            ))
        }
    }
}
