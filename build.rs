use std::{env, path::PathBuf, process::Command};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Recognize Tarpaulin cfgs for conditional compilation without warnings
    println!("cargo:rustc-check-cfg=cfg(tarpaulin)");
    println!("cargo:rustc-check-cfg=cfg(tarpaulin_include)");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    let out_dir = PathBuf::from(&manifest_dir).join("src/proto");
    tonic_build::configure()
        .out_dir(&out_dir)
        .compile_protos(&["proto/simpledb.proto"], &["proto"])?;

    lalrpop::Configuration::new()
        .generate_in_source_tree()
        .process()?;

    // Format auto-generated sources.
    Command::new("cargo")
        .current_dir(&manifest_dir)
        .args([
            "fmt",
            "--",
            "src/proto/simpledb.rs",
            "src/parser/grammar.rs",
        ])
        .status()?;

    Ok(())
}
