fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/keyward.proto");
    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile_protos(&["proto/keyward.proto"], &["proto"])?;
    Ok(())
}
