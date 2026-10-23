//! rowan: a whole-program abstract interpreter for Python. It parses a program
//! (`frontend.rs`, `scopes.rs`, `syntax.rs`), runs it on abstract values (`domain.rs`) and
//! states (`state.rs`, over the persistent maps of `layered.rs` and the interned ids of
//! `intern.rs`) in the interpreter (`interp/`), with the library models of `models/`, and
//! reports what it found (`interp/report.rs`). `README.md` in this folder is the map.
pub mod domain;
pub mod frontend;
pub mod intern;
pub mod interp;
pub mod layered;
#[cfg(test)]
mod laws;
pub mod models;
pub mod scopes;
pub mod source;
pub mod state;
pub mod syntax;

pub use interp::report;
