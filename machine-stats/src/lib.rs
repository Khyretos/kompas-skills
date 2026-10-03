//! Linux machine stats from /proc and sysfs, shared by the Kompanion server
//! (its own machine) and the runner (every other PC). No root needed.

pub mod fdinfo;
pub mod gpu;
pub mod system;

pub use gpu::{GpuReader, GpuStats};
pub use system::{Sampler, Snapshot};
