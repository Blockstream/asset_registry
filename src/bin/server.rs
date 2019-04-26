#![feature(proc_macro_hygiene, decl_macro)]

extern crate asset_registry;
extern crate stderrlog;
#[macro_use]
extern crate log;

use std::path::PathBuf;

use structopt::StructOpt;

use asset_registry::errors::Result;
use asset_registry::start_server;

#[derive(StructOpt, Debug)]
struct Config {
    #[structopt(
        short,
        long,
        parse(from_occurrences),
        help = "Increase verbosity (up to 3)"
    )]
    verbose: usize,
    #[structopt(short, long = "db-path", help = "Path to database directory")]
    db_path: PathBuf,
}

fn main() -> Result<()> {
    let config = Config::from_args();
    stderrlog::new()
        .verbosity(config.verbose + 2)
        .init()
        .unwrap();
    info!("Server config: {:?}", config);

    start_server(&config.db_path)?.launch();
    info!("HTTP server started");

    Ok(())
}
