#![feature(proc_macro_hygiene, decl_macro)]

extern crate asset_registry;
extern crate stderrlog;

use structopt::StructOpt;

use asset_registry::errors::Result;
use asset_registry::server::{start_server, Config};

fn main() -> Result<()> {
    let config = Config::from_args();
    start_server(config)?.launch();

    Ok(())
}
