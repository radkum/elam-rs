use std::{env, fs, path::Path};

const HASH_LEN: usize = 32;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // wdk-build 0.2.x API (newer releases use `configure_wdk_binary_build`).
    wdk_build::Config::from_env_auto()?.configure_binary_build();

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

fn read_hashes(path: &Path) -> Result<Vec<[u8; HASH_LEN]>, Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed={}", path.display());

    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };

    let mut hashes = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        let hex: String =
            line.chars().filter(|c| !c.is_whitespace() && *c != ',' && *c != ':').collect();

        let hash = decode_sha256(&hex).ok_or_else(|| {
            format!(
                "{}:{}: invalid SHA-256 digest (expected 64 hex chars): {:?}",
                path.display(),
                index + 1,
                raw.trim()
            )
        })?;
        hashes.push(hash);
    }

    Ok(hashes)
}

fn decode_sha256(hex: &str) -> Option<[u8; HASH_LEN]> {
    if hex.len() != HASH_LEN * 2 {
        return None;
    }

    let mut out = [0u8; HASH_LEN];
    let bytes = hex.as_bytes();
    for (i, out_byte) in out.iter_mut().enumerate() {
        let hi = (bytes[i * 2] as char).to_digit(16)?;
        let lo = (bytes[i * 2 + 1] as char).to_digit(16)?;
        *out_byte = (hi * 16 + lo) as u8;
    }
    Some(out)
}

fn write_table(src: &mut String, name: &str, hashes: &[[u8; HASH_LEN]]) {
    use std::fmt::Write as _;

    let _ = writeln!(src, "pub(crate) const {name}: [[u8; {HASH_LEN}]; {}] = [", hashes.len());
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
