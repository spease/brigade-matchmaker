#![deny(missing_docs)]
//! Utility to dump database
#[macro_use]
extern crate derive_more;
extern crate failure;
extern crate heck;
extern crate itertools;
extern crate mongodb;
#[macro_use]
extern crate quicli;
extern crate serde;
extern crate serde_derive;
extern crate titlecase;
extern crate toml;

use heck::KebabCase;

use itertools::Itertools;

use mongodb::coll::Collection;
use mongodb::db::ThreadedDatabase;
use mongodb::{Client, ThreadedClient};

use quicli::prelude::*;

use std::collections::HashMap;
use std::iter::FromIterator;
use std::str::FromStr;

use titlecase::titlecase;

#[derive(Debug, StructOpt)]
struct Cli {
    #[structopt(
        default_value = "projecttaxonomies",
        help = "collection to use",
        long = "collection",
        short = "c"
    )]
    collection: String,

    #[structopt(
        default_value = "localhost",
        help = "mongodb host to connect to",
        long = "host",
        short = "h"
    )]
    host: String,

    #[structopt(default_value = "27017", help = "server port", long = "port")]
    port: u16,

    #[structopt(
        default_value = "brigade_matchmaker",
        help = "database to use",
        long = "db",
        short = "d"
    )]
    db: String,

    #[structopt(flatten)]
    verbosity: Verbosity,
}

/// A taxonomy entry
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaxonomyEntry {
    /// Class of this taxonomy entry
    pub class_name: String,
    /// Identifier for this taxonomy entry
    pub name: TaxonomyIdentifier,
    /// Parent identifier of this taxonomy entry
    pub parent: Option<TaxonomyIdentifier>,
    /// Synonyms for this taxonomy entry
    pub synonyms: Vec<String>,
    /// Title for this taxonomy entry
    pub title: TaxonomyTitle,
}

/// A kebab-case identifier that cannot contain a slash
#[derive(Clone, Debug, Display, Eq, Hash, PartialEq, Serialize)]
pub struct TaxonomyIdentifier(String);

impl<'de> serde::de::Deserialize<'de> for TaxonomyIdentifier {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl FromStr for TaxonomyIdentifier {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s.contains("/") {
            Err(format_err!("Cannot contain slash"))?;
        }
        if s.to_kebab_case() != s {
            Err(format_err!("Identifier {:?} is not kebab-case", s))?;
        }
        Ok(TaxonomyIdentifier(s.to_owned()))
    }
}

/// A path of TaxonomyIdentifiers
#[derive(Debug, Display, Eq, Hash, PartialEq, Serialize)]
pub struct TaxonomyPath(String);

impl<'a> FromIterator<&'a TaxonomyIdentifier> for TaxonomyPath {
    fn from_iter<I: IntoIterator<Item = &'a TaxonomyIdentifier>>(iter: I) -> Self {
        TaxonomyPath(iter.into_iter().join("/"))
    }
}

/// A title using title case
#[derive(Clone, Debug, Serialize)]
pub struct TaxonomyTitle(String);

impl<'de> serde::de::Deserialize<'de> for TaxonomyTitle {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl FromStr for TaxonomyTitle {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let s_titlecase = titlecase(s);
        if s_titlecase != s {
            warn!("Title {:?} should use title case {:?}", s, s_titlecase);
        }
        let s_trimmed = s.trim();
        if s_trimmed != s {
            Err(format_err!("Title {:?} has preceding or trailing spaces", s))?;
        }
        Ok(TaxonomyTitle(s.to_owned()))
    }
}

/// Represents a taxonomy collection loaded from a mongodb collection
#[derive(Debug)]
pub struct TaxonomyCollection(HashMap<TaxonomyIdentifier, TaxonomyEntry>);

impl TaxonomyCollection {
    /// Reads a TaxonomyEntry collection from a mongodb collection
    pub fn from_collection(collection: &Collection) -> Result<TaxonomyCollection> {
        let cursor = collection.find(None, None)?;
        let taxonomies_hash = cursor
            .map(|item_result| {
                item_result.map_err(Error::from).and_then(|item| {
                    Ok(TaxonomyEntry {
                        name: item.get_str("name")?.parse()?,
                        parent: {
                            if item.is_null("parent") {
                                None
                            } else {
                                Some(item.get_str("parent")?.parse()?)
                            }
                        },
                        class_name: item.get_str("className")?.parse()?,
                        title: item.get_str("title")?.parse()?,
                        synonyms: item
                            .get_array("synonyms")?
                            .into_iter()
                            .map(|x| {
                                x.as_str()
                                    .map(|s| s.to_owned())
                                    .ok_or(format_err!("Invalid type"))
                            }).collect::<Result<Vec<String>>>()?,
                    })
                })
            }).map_results(|x| (x.name.clone(), x))
            .collect::<Result<HashMap<TaxonomyIdentifier, TaxonomyEntry>>>()?;

        Ok(TaxonomyCollection(taxonomies_hash))
    }

    /// Provides the absolute path of a specified taxonomy entry
    fn full_path(&self, t: &TaxonomyEntry) -> Result<TaxonomyPath> {
        let mut path = vec![&t.name];
        let mut current = t;
        while let Some(ref parent_name) = current.parent {
            if parent_name == &current.name {
                warn!(
                    "Parent loop detected for entry '{}' - assuming None",
                    t.name
                );
                break;
            }
            current = self
                .0
                .get(parent_name)
                .ok_or(format_err!("Missing '{}'", parent_name))?;
            path.push(&current.name);
        }
        Ok(path.into_iter().rev().collect())
    }

    /// Creates an editable version of the taxonomy collection
    pub fn to_editable(&self) -> Result<TaxonomyCollectionEditable> {
        let r = self
            .0
            .values()
            .map(|x| self.full_path(x).map(|fp| (fp.to_string(), x.clone())))
            .collect::<Result<_>>()?;

        Ok(TaxonomyCollectionEditable(r))
    }
}

/// Represents an editable form of a taxonomy collection
#[derive(Debug)]
pub struct TaxonomyCollectionEditable(HashMap<String, TaxonomyEntry>);

impl TaxonomyCollectionEditable {
    /// Convert to a TOML string
    pub fn to_toml_string(&self) -> Result<String> {
        Ok(toml::ser::to_string(&self.0)?)
    }
}

main!(|args: Cli, log_level: verbosity| {
    // Connect to server
    let client = Client::connect(&args.host, args.port).context(format!(
        "Failed to connect to mongodb server at {}:{}",
        args.host, args.port
    ))?;

    // Get database handle if it exists
    let db = {
        let database_names = client.database_names()?;
        if !database_names.contains(&args.db) {
            Err(format_err!("No database {:?} on server {:?}:{:?}. Found databases: {:?}", &args.db, &args.host, args.port, database_names))?;
        }
        client.db(&args.db)
    };

    // Get collection handle if it exists
    let collection = {
        let collection_names = db.collection_names(None)?;
        if !collection_names.contains(&args.collection) {
            Err(format_err!("No collection {:?} in database {:?} on server {:?}:{:?}. Found collections: {:?}", &args.collection, &args.db, &args.host, args.port, collection_names))?;
        }
        db.collection(&args.collection)
    };

    // Extract the data from the collection to memory
    let taxonomy_collection = TaxonomyCollection::from_collection(&collection)
        .context(format!("Failed to extract collection"))?;

    // Convert the data to an ediable form
    let taxonomy_collection_editable = taxonomy_collection
        .to_editable()
        .context("Failed to convert taxonomy collection to editable form")?;

    // Convert it to a TOML string
    let taxonomy_toml_string = taxonomy_collection_editable.to_toml_string()
        .context("Failed to convert taxonomy collection to TOML")?;

    println!("{}", taxonomy_toml_string);
});
