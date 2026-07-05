//! Variable scoping, interpolation and dynamic values.
//!
//! Precedence (highest wins):
//! runtime > request > folder (inner > outer) > environment (+secrets) >
//! collection > process env / .env

pub mod dotenv;
pub mod dynamic;
pub mod interpolate;
pub mod path;
pub mod scope;

pub use dotenv::{load_dotenv, process_env_snapshot};
pub use interpolate::{Interpolated, Warning, interpolate};
pub use scope::{Scope, StackInputs, VarStack};
