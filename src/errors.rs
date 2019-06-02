use std::result::Result as StdResult;

use core::fmt::Display;
pub use failure::{Context, Error, ResultExt};

pub type Result<T> = StdResult<T, Error>;

pub trait OptionExt<T> {
    fn or_err<D>(self, context: D) -> StdResult<T, Context<D>>
    where
        D: Display + Send + Sync + 'static;

    fn req(self) -> StdResult<T, Context<&'static str>>;
}

impl<T> OptionExt<T> for Option<T> {
    fn or_err<D>(self, context: D) -> StdResult<T, Context<D>>
    where
        D: Display + Send + Sync + 'static,
    {
        self.ok_or_else(|| Context::new(context))
    }

    fn req(self) -> StdResult<T, Context<&'static str>> {
        self.ok_or_else(|| Context::new("missing required option"))
    }
}

pub fn join_err(err: &Error) -> String {
    err.iter_chain()
        .map(|s| s.to_string())
        .collect::<Vec<String>>()
        .join(": ")
}
