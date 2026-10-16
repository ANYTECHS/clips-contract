//! Contract storage accessors.
//!
//! Groups instance-storage reads/writes (e.g. [`config`]) behind one module
//! tree so entry points never touch raw storage keys directly.

pub mod config;
