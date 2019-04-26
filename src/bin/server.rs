#![feature(proc_macro_hygiene, decl_macro)]

extern crate asset_registry;
extern crate stderrlog;

use std::path::Path;

use asset_registry::start_server;
use asset_registry::errors::Result;

fn main() -> Result<()> {
    stderrlog::new().verbosity(3).init().unwrap();
    // TODO make path configurable
    start_server(&Path::new("./db"))?.launch();
    Ok(())
}
