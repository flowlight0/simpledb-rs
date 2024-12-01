use std::{env, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("src/proto");
    tonic_build::configure()
        .out_dir(out_dir)
        .compile_protos(&["proto/simpledb.proto"], &["proto"])?;

    lalrpop::Configuration::new()
        .generate_in_source_tree()
        .process()
}
