fn main() -> Result<(), Box<dyn std::error::Error>> {
    wdk_build::configure_wdk_binary_build()?;

    let resource_name = std::env::var("CARGO_PKG_NAME")?.replace("-", "_");
    //let resource_name = resource_name.strip_suffix("_rs")?;
    println!("cargo:rerun-if-changed={resource_name}");
    embed_resource::compile(resource_name, embed_resource::NONE);
    Ok(())
}
