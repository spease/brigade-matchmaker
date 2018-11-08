extern crate bson;
extern crate derive_more;
extern crate failure;
extern crate heck;
extern crate itertools;
extern crate log;
extern crate mongodb;
extern crate serde;
extern crate serde_derive;
extern crate titlecase;
extern crate toml;

use derive_more::Display;

use failure::{format_err, Error};

use heck::KebabCase;

use itertools::Itertools;

use log::warn;

use bson::{Bson, Document};

use serde_derive::{Deserialize, Serialize};

use std::collections::HashMap;
use std::iter::FromIterator;
use std::str::FromStr;

use titlecase::titlecase;

type Result<T> = ::std::result::Result<T, Error>;

/// A taxonomy entry
#[derive(Clone, Debug)]
pub struct CollectionEntry {
    /// Class of this taxonomy entry
    pub class_name: String,
    /// Identifier for this taxonomy entry
    pub name: Identifier,
    /// Parent identifier of this taxonomy entry
    pub parent: Option<Identifier>,
    /// Synonyms for this taxonomy entry
    pub synonyms: Vec<String>,
    /// Title for this taxonomy entry
    pub title: Title,
}

impl Into<Document> for CollectionEntry {
    fn into(self) -> Document {
        let mut d = Document::new();
        d.insert("name", self.name.to_string());
        d.insert(
            "parent",
            self.parent
                .map(|p| Bson::String(p.to_string()))
                .unwrap_or(Bson::Null),
        );
        d.insert("className", self.class_name);
        d.insert("title", self.title.to_string());
        d.insert(
            "synonyms",
            self.synonyms
                .into_iter()
                .map(|s| Bson::String(s))
                .collect::<Vec<Bson>>(),
        );
        d
    }
}

/// A map entry
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MapEntry {
    /// Class of this taxonomy entry
    pub class_name: String,
    /// Keywords for this taxonomy entry
    pub synonyms: Vec<String>,
    /// Human-readable title for this taxonomy entry
    pub title: Title,
}

/// A kebab-case identifier that cannot contain a slash
#[derive(Clone, Debug, Display, Eq, Hash, PartialEq, Serialize)]
pub struct Identifier(String);

impl<'de> serde::de::Deserialize<'de> for Identifier {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl FromStr for Identifier {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s.is_empty() {
            Err(format_err!("Empty identifier"))?;
        }
        if s.contains('/') {
            Err(format_err!("Identifier {:?} contain a slash", s))?;
        }
        if s.to_kebab_case() != s {
            Err(format_err!("Identifier {:?} is not kebab-case", s))?;
        }
        Ok(Identifier(s.to_owned()))
    }
}

/// A path of Identifiers
#[derive(Debug, Display, Eq, Hash, PartialEq, Serialize)]
pub struct Path(String);

impl Path {
    const SEPARATOR: char = '/';
    fn name(&self) -> Option<Identifier> {
        self.0
            .split(Self::SEPARATOR)
            .last()
            .map(|s| Identifier(s.to_owned()))
    }

    fn parent(&self) -> Option<Identifier> {
        self.0
            .split(Self::SEPARATOR)
            .rev()
            .nth(1)
            .map(|s| Identifier(s.to_owned()))
    }
}

impl FromIterator<Identifier> for Path {
    fn from_iter<I: IntoIterator<Item = Identifier>>(iter: I) -> Self {
        Path(iter.into_iter().join("/"))
    }
}

impl<'a> FromIterator<&'a Identifier> for Path {
    fn from_iter<I: IntoIterator<Item = &'a Identifier>>(iter: I) -> Self {
        Path(iter.into_iter().join("/"))
    }
}

impl<'de> serde::de::Deserialize<'de> for Path {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl FromStr for Path {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(s.split(Self::SEPARATOR)
            .map(|x| x.parse())
            .collect::<Result<Vec<Identifier>>>()?
            .into_iter()
            .collect())
    }
}

/// A title using title case
#[derive(Clone, Debug, Display, Serialize)]
pub struct Title(String);

impl<'de> serde::de::Deserialize<'de> for Title {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl FromStr for Title {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let s_titlecase = titlecase(s);
        if s_titlecase != s {
            warn!("Title {:?} should use title case {:?}", s, s_titlecase);
        }
        let s_trimmed = s.trim();
        if s_trimmed != s {
            Err(format_err!(
                "Title {:?} has preceding or trailing spaces",
                s
            ))?;
        }
        Ok(Title(s.to_owned()))
    }
}

/// Represents a taxonomy collection loaded from a mongodb collection
#[derive(Debug)]
pub struct Collection(HashMap<Identifier, CollectionEntry>);

impl Collection {
    /// Reads a CollectionEntry collection from a mongodb collection
    pub fn from_collection(collection: &mongodb::coll::Collection) -> Result<Collection> {
        let cursor = collection.find(None, None)?;
        let taxonomies_hash = cursor
            .map(|item_result| {
                item_result.map_err(Error::from).and_then(|item| {
                    Ok(CollectionEntry {
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
                                    .ok_or_else(|| format_err!("Invalid type"))
                            }).collect::<Result<Vec<String>>>()?,
                    })
                })
            }).map_results(|x| (x.name.clone(), x))
            .collect::<Result<HashMap<Identifier, CollectionEntry>>>()?;

        Ok(Collection(taxonomies_hash))
    }

    /// Writes a collection to a mongodb collection
    pub fn to_collection(&self, collection: &mongodb::coll::Collection) -> Result<()> {
        collection.delete_many(Document::new(), None)?;
        collection.insert_many(self.0.values().map(|entry| entry.clone().into()).collect(), None)?;
        Ok(())
    }

    /// Provides the absolute path of a specified taxonomy entry
    fn full_path(&self, t: &CollectionEntry) -> Result<Path> {
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
                .ok_or_else(|| format_err!("Missing '{}'", parent_name))?;
            path.push(&current.name);
        }
        Ok(path.into_iter().rev().collect())
    }

    /// Creates an editable version of the taxonomy collection
    pub fn to_map(&self) -> Result<Map> {
        let r = self
            .0
            .values()
            .map(|x| {
                self.full_path(x).map(|fp| {
                    (
                        fp,
                        MapEntry {
                            title: x.title.clone(),
                            synonyms: x.synonyms.clone(),
                            class_name: x.class_name.clone(),
                        },
                    )
                })
            }).collect::<Result<_>>()?;

        Ok(Map(r))
    }
}

/// Represents an editable form of a taxonomy collection
#[derive(Debug, Deserialize, Serialize)]
pub struct Map(HashMap<Path, MapEntry>);

impl Map {
    pub fn into_collection(self) -> Result<Collection> {
        Ok(Collection(
            self.0
                .into_iter()
                .map(|(path, entry)| {
                    let path_name = path
                        .name()
                        .ok_or_else(|| format_err!("Missing path name for {:?}", entry))?;
                    Ok((
                        path_name.clone(),
                        CollectionEntry {
                            class_name: entry.class_name,
                            name: path_name,
                            parent: path.parent(),
                            synonyms: entry.synonyms,
                            title: entry.title,
                        },
                    ))
                }).collect::<Result<_>>()?,
        ))
    }
}
