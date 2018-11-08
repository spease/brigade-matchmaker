#![deny(missing_docs)]
//! Utility to dump database
extern crate failure;
extern crate mongodb;
#[macro_use]
extern crate quicli;
extern crate serde_json;
extern crate toml;

use mongodb::db::ThreadedDatabase;
use mongodb::{Client, ThreadedClient};

use quicli::prelude::*;

#[derive(Debug, StructOpt)]
enum Command {
    #[structopt(name = "load")]
    Load {
        #[structopt(subcommand)]
        format: Format,
    },
    #[structopt(name = "store")]
    Store {
        #[structopt(subcommand)]
        format: Format,
    },
}

#[derive(Debug, StructOpt)]
enum Format {
    #[structopt(name = "json")]
    Json,
    #[structopt(name = "toml")]
    Toml,
}

#[derive(Debug, StructOpt)]
struct Cli {
    #[structopt(
        default_value = "projecttaxonomies",
        help = "collection to use",
        long = "collection",
        short = "c"
    )]
    collection: String,

    #[structopt(subcommand)]
    command: Command,

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

main!(|args: Cli, log_level: verbosity| {
    // Connect to server
    let client = Client::connect(&args.host, args.port).context(format!(
        "Failed to connect to mongodb server at {}:{}",
        args.host, args.port
    ))? ;

    // Get database handle if it exists
    let db = {
        let database_names = client.database_names()?;
        if !database_names.contains(&args.db) {
            Err(format_err!(
                "No database {:?} on server {:?}:{:?}. Found databases: {:?}",
                &args.db,
                &args.host,
                args.port,
                database_names
            ))?;
        }
        client.db(&args.db)
    };

    // Get collection handle if it exists
    let collection = {
        let collection_names = db.collection_names(None)?;
        if !collection_names.contains(&args.collection) {
            Err(format_err!(
                "No collection {:?} in database {:?} on server {:?}:{:?}. Found collections: {:?}",
                &args.collection,
                &args.db,
                &args.host,
                args.port,
                collection_names
            ))?;
        }
        db.collection(&args.collection)
    };

    match args.command {
        Command::Load{format} => {

            // Extract the data from the collection to memory
            let taxonomy_collection = taxonomy::Collection::from_collection(&collection)
                .context("Failed to extract collection from database")?;

            // Convert the data to an ediable form
            let taxonomy_collection_editable = taxonomy_collection
                .to_map()
                .context("Failed to serialize taxonomy collection to stdout")?;

            // Convert it to a string
            let taxonomy_string = match format {
                Format::Toml => toml::ser::to_string(&taxonomy_collection_editable).map_err(Error::from),
                Format::Json => serde_json::to_string_pretty(&taxonomy_collection_editable).map_err(Error::from),
            }.context("Failed to convert taxonomy collection to TOML")?;

            println!("{}", taxonomy_string);
        }
        Command::Store{format} => {
            use std::io::Read;

            let mut s = String::new();
            let stdin = std::io::stdin();
            stdin.lock().read_to_string(&mut s)?;

            let taxonomy_map: taxonomy::Map = match format {
                Format::Toml => toml::de::from_str(&s).map_err(Error::from),
                Format::Json => serde_json::from_str(&s).map_err(Error::from),
            }.context("Failed to deserialize taxonomy map from stdin")?;

            let taxonomy_collection = taxonomy_map.into_collection().context("Failed to pack taxonomy map into collection format")?;
            // taxonomy_collection.to_collection(&collection)?;
        },
    }
});
