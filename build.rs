//! Build script for SVLM
//!
//! Previously compiled Protocol Buffer definitions, but now uses the official
//! Yellowstone gRPC client which handles protobuf compilation internally.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // No custom protobuf compilation needed - using official Yellowstone gRPC client
    println!("cargo:rerun-if-changed=Cargo.toml");
    Ok(())
}