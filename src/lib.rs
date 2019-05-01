#![feature(proc_macro_hygiene, decl_macro)]

extern crate base64;
extern crate bitcoin;
extern crate elements;
extern crate secp256k1;
extern crate serde;
#[macro_use]
extern crate base64_serde;
#[macro_use]
extern crate serde_derive;
#[cfg(feature = "server")]
#[macro_use]
extern crate rocket;
#[cfg(feature = "server")]
extern crate rocket_contrib;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
extern crate regex;
#[cfg(feature = "cli")]
extern crate structopt;

pub mod asset;
pub mod entity;
pub mod errors;
pub mod registry;
#[cfg(feature = "server")]
pub mod server;
pub mod util;
