//! Isolated current storage source host: avoids unrelated historical IO library fixture compilation.
#![forbid(unsafe_code)]
// These actual private modules include unused portability APIs; their algorithms are not copied.
#[allow(dead_code, unused_imports)]
#[path = "../src/filesystem/mod.rs"]
mod filesystem;
#[allow(dead_code)]
#[path = "../src/video_output.rs"]
mod video_output;

#[allow(dead_code)]
#[path = "../src/sequence.rs"]
mod sequence;
