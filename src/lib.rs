#![cfg_attr(test, feature(proc_macro_hygiene, decl_macro))]

extern crate base64;
extern crate bitcoin;
extern crate elements;
extern crate secp256k1;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
extern crate regex;

#[cfg(feature = "server")]
extern crate hyper;
#[cfg(feature = "cli")]
extern crate structopt;

#[cfg(test)]
#[macro_use]
extern crate rocket;
#[cfg(test)]
extern crate rocket_contrib;

pub mod asset;
pub mod chain;
#[cfg(feature = "client")]
pub mod client;
pub mod entity;
pub mod errors;
pub mod registry;
#[cfg(feature = "server")]
pub mod server;
pub mod util;
