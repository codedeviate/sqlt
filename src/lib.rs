//! `sqlt` — multi-dialect SQL parser and translator.
//!
//! See `CLAUDE.md` for the project guide and `README.md` for usage.

pub mod ast;
pub mod cli;
pub mod dialect;
pub mod emit;
pub mod encoding;
pub mod error;
pub mod json;
pub mod parse;
pub mod translate;
