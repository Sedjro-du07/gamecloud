//! Database access layer.
//!
//! All sqlx queries live in submodules of `queries`. Higher layers
//! never touch `sqlx` types directly — they call functions defined
//! here.

pub mod pool;
pub mod queries;
