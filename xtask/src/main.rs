use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const DRIVER_NAME: &str = "elam_rs";

fn main() {
    let task = env::args().nth(1).unwrap_or_else(|| "all".into());
    let release = env::args().any(|a| a == "--release");

    let result = match task.as_str() {
        "fmt" | "format" => fmt(),
        "resources" => resources(),
        "build" => build(release),
        "rename" => rename(release),
        "sign" => sign(release),
        "all" => all(release),
        "rebuild" => rebuild(release),
        "clean" => clean(),
        "stage" => stage(release),
        "ppl-test" => ppl_test(release),
        _ => {
            eprintln!("Unknown task: {task}");
            print_help();
            std::process::exit(1);
        },
    };

    if let Err(e) = result {
        eprintln!("xtask failed: {e}");
        std::process::exit(1);
    }
}

fn fmt() -> Result<()> {
    cargo(&["fmt"])
}

fn resources() -> Result<()> {
    let status = Command::new("powershell")
        .args(["-ExecutionPolicy", "Bypass", "-File", ".\\generate_cert.ps1"])
        .current_dir(workspace_root())
        .status()?;
    if !status.success() {
        return Err(format!("generate_cert.ps1 failed with {status}").into());
    }
    Ok(())
}

fn build(release: bool) -> Result<()> {
    fmt()?;
    let mut args = vec!["build", "--package", "elam-rs"];
    if release {
        args.push("--release");
    }
    cargo(&args)
}

fn rename(release: bool) -> Result<()> {
    let dir = target_dir(release);
    let dll = dir.join(format!("{DRIVER_NAME}.dll"));
    let sys = dir.join(format!("{DRIVER_NAME}.sys"));
    if sys.exists() {
        std::fs::remove_file(&sys)?;
    }
    std::fs::rename(&dll, &sys)
        .map_err(|e| format!("rename failed ({} -> {}): {e}", dll.display(), sys.display()).into())
}

fn sign(release: bool) -> Result<()> {
    let sys = target_dir(release).join(format!("{DRIVER_NAME}.sys"));
    sign_file(&sys)
}

fn sign_file(path: &Path) -> Result<()> {
    let pfx = workspace_root().join(format!("{DRIVER_NAME}.pfx"));
    if !pfx.exists() {
        return Err("elam_rs.pfx not found — run `cargo xtask resources` first".into());
    }
    let signtool = find_signtool()?;
    run(
        &signtool.to_string_lossy(),
        &[
            "sign",
            "/fd",
            "SHA256",
            "/a",
            "/v",
            "/ph",
            "/f",
            pfx.to_str().unwrap(),
            "/p",
            "password",
            "/t",
            "http://timestamp.digicert.com",
            path.to_str().unwrap(),
        ],
    )
}

fn ppl_test(release: bool) -> Result<()> {
    let mut args = vec!["build", "--package", "ppl-test"];
    if release {
        args.push("--release");
    }
    cargo(&args)?;
    let exe = target_dir(release).join("ppl_test.exe");
    sign_file(&exe)
}

fn all(release: bool) -> Result<()> {
    resources()?;
    build(release)?;
    rename(release)?;
    sign(release)
}

fn rebuild(release: bool) -> Result<()> {
    clean()?;
    all(release)
}

fn stage(release: bool) -> Result<()> {
    let src = target_dir(release);
    let root = workspace_root();
    let stage = root.join("target").join("stage");
    std::fs::create_dir_all(&stage).map_err(|e| format!("failed to create target/stage: {e}"))?;

    let files: &[(&str, Option<&str>)] = &[
        (&format!("{DRIVER_NAME}.sys"), None),
        (&format!("{DRIVER_NAME}.pfx"), Some("")), // source: workspace root
        ("ppl_test.exe", None),
        ("install.ps1", Some("")),
        ("diagnose.ps1", Some("")),
    ];

    for (name, root_override) in files {
        let from = match root_override {
            Some(_) => root.join(name),
            None => src.join(name),
        };
        if !from.exists() {
            return Err(format!("{} not found — build first", from.display()).into());
        }
        std::fs::copy(&from, stage.join(name))
            .map_err(|e| format!("failed to copy {name}: {e}"))?;
    }

    println!("Staged to: {}", stage.display());
    println!("Copy the contents to the test VM and run install.ps1 as Administrator.");
    Ok(())
}

fn clean() -> Result<()> {
    cargo(&["clean", "--package", "elam-rs"])
}

fn cargo(args: &[&str]) -> Result<()> {
    run("cargo", args)
}

fn run(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program).args(args).status()?;
    if !status.success() {
        return Err(format!("`{program} {}` failed with {status}", args.join(" ")).into());
    }
    Ok(())
}

fn target_dir(release: bool) -> PathBuf {
    let profile = if release { "release" } else { "debug" };
    let base = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root().join("target"));
    base.join(profile)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask has no parent directory")
        .to_owned()
}

fn find_signtool() -> Result<PathBuf> {
    if let Ok(out) = Command::new("where").arg("signtool.exe").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            let first = s.lines().next().unwrap_or("").trim();
            if !first.is_empty() {
                return Ok(PathBuf::from(first));
            }
        }
    }

    let sdk_bin = PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\bin");
    if sdk_bin.exists() {
        let mut versions: Vec<PathBuf> = std::fs::read_dir(&sdk_bin)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        versions.sort_unstable_by(|a, b| b.cmp(a));
        for ver in versions {
            let candidate = ver.join("x64").join("signtool.exe");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    Err("signtool.exe not found — install Windows SDK or add it to PATH".into())
}

fn print_help() {
    eprintln!(
        "Usage: cargo xtask <task> [--release]

Tasks:
  fmt / format   Format code with cargo fmt
  resources      Generate signing certificate via generate_cert.ps1
  build          fmt + compile the driver
  rename         Rename {DRIVER_NAME}.dll -> {DRIVER_NAME}.sys
  sign           Sign the driver with signtool.exe
  all            resources -> build -> rename -> sign  [default]
  rebuild        clean -> all
  clean          cargo clean
  stage          Collect driver + test artifacts into target/stage/ (copy to VM)
  ppl-test       Build + sign ppl_test.exe (service for PPL anchor testing)"
    );
}
