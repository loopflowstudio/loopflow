fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    std::env::set_var("PROTOC", protoc);
    tonic_build::configure().build_server(true).compile(
        &["../../proto/loopflow/control/v1/control.proto"],
        &["../../proto"],
    )?;
    Ok(())
}
