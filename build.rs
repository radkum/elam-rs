use std::{env, fs, path::Path};

use elam_core::SHA256_LEN;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    wdk_build::configure_wdk_binary_build()?;

    generate_hash_tables()?;

    let resource_name = env::var("CARGO_PKG_NAME")?.replace("-", "_");
    println!("cargo:rerun-if-changed={resource_name}");
    embed_resource::compile(resource_name, embed_resource::NONE);
    Ok(())
}

/// Read `hashes/known_good.txt` and `hashes/known_bad.txt` and emit
/// `$OUT_DIR/known_hashes.rs` with the `KNOWN_GOOD_HASHES` / `KNOWN_BAD_HASHES`
/// tables consumed by `src/enums.rs`.
fn generate_hash_tables() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = Path::new(&env::var("CARGO_MANIFEST_DIR")?).to_path_buf();
    let out_dir = Path::new(&env::var("OUT_DIR")?).to_path_buf();

    let good = read_hashes(&manifest.join("hashes").join("known_good.txt"))?;
    let bad = read_hashes(&manifest.join("hashes").join("known_bad.txt"))?;

    let mut src = String::new();
    write_table(&mut src, "KNOWN_GOOD_HASHES", &good);
    write_table(&mut src, "KNOWN_BAD_HASHES", &bad);

    fs::write(out_dir.join("known_hashes.rs"), src)?;
    Ok(())
}

fn read_hashes(path: &Path) -> Result<Vec<[u8; SHA256_LEN]>, Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed={}", path.display());

    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };

    // Parsing/validation lives in `elam-core` so it is unit-tested on the host.
    elam_core::parse_hash_lines(&text)
        .map_err(|err| format!("{}:{}: {err}", path.display(), err.line).into())
}

fn write_table(src: &mut String, name: &str, hashes: &[[u8; SHA256_LEN]]) {
    use std::fmt::Write as _;

    let _ = writeln!(src, "pub(crate) const {name}: [[u8; {SHA256_LEN}]; {}] = [", hashes.len());
    for hash in hashes {
        src.push_str("    [");
        for (i, byte) in hash.iter().enumerate() {
            if i > 0 {
                src.push_str(", ");
            }
            let _ = write!(src, "0x{byte:02x}");
        }
        src.push_str("],\n");
    }
    src.push_str("];\n");
}
