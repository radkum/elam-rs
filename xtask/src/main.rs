use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const DRIVER_NAME: &str = "elam_rs";

fn main() {
    let (task, release) = parse_cli(&env::args().collect::<Vec<_>>());

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
    let script = workspace_root().join("generate_cert.ps1");

    // Windows PowerShell 5.1 can fail to autoload `Microsoft.PowerShell.Security`
    // when it inherits a `PSModulePath` that includes PowerShell 7 / third-party
    // module directories. The cert script only needs built-in cmdlets, so hand
    // the child a clean, system-only module path.
    let system_modules = match std::env::var("SystemRoot") {
        Ok(root) => format!(r"{root}\System32\WindowsPowerShell\v1.0\Modules"),
        Err(_) => r"C:\Windows\System32\WindowsPowerShell\v1.0\Modules".to_owned(),
    };

    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .current_dir(workspace_root())
        .env("PSModulePath", system_modules)
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

    for item in stage_manifest(DRIVER_NAME) {
        let from = if item.from_root { root.join(&item.name) } else { src.join(&item.name) };
        if !from.exists() {
            return Err(format!("{} not found — build first", from.display()).into());
        }
        std::fs::copy(&from, stage.join(&item.name))
            .map_err(|e| format!("failed to copy {}: {e}", item.name))?;
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
    let base = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root().join("target"));
    profile_dir(&base, release)
}

fn profile_dir(base: &Path, release: bool) -> PathBuf {
    base.join(if release { "release" } else { "debug" })
}

/// Parse `<program> [task] [--release...]`. Leading flags are skipped, so
/// `cargo xtask --release` means the default `all` task in release mode.
fn parse_cli(args: &[String]) -> (String, bool) {
    let release = args.iter().skip(1).any(|arg| arg == "--release");
    let task = args
        .iter()
        .skip(1)
        .find(|arg| !arg.starts_with("--"))
        .cloned()
        .unwrap_or_else(|| "all".to_owned());
    (task, release)
}

struct StageItem {
    name: String,
    /// Source is the workspace root (scripts, cert, INF) rather than the target
    /// profile directory (built binaries).
    from_root: bool,
}

/// Files collected into `target/stage/`.
fn stage_manifest(driver_name: &str) -> Vec<StageItem> {
    vec![
        StageItem { name: format!("{driver_name}.sys"), from_root: false },
        StageItem { name: format!("{driver_name}.pfx"), from_root: true },
        StageItem { name: "ppl_test.exe".to_owned(), from_root: false },
        StageItem { name: "install.ps1".to_owned(), from_root: true },
        StageItem { name: "uninstall.ps1".to_owned(), from_root: true },
        StageItem { name: "diagnose.ps1".to_owned(), from_root: true },
        StageItem { name: format!("{driver_name}.inf"), from_root: true },
    ]
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parse_cli_defaults_to_all() {
        assert_eq!(parse_cli(&args(&["xtask"])), ("all".to_owned(), false));
    }

    #[test]
    fn parse_cli_release_without_task_means_all() {
        assert_eq!(parse_cli(&args(&["xtask", "--release"])), ("all".to_owned(), true));
    }

    #[test]
    fn parse_cli_task_and_release() {
        assert_eq!(
            parse_cli(&args(&["xtask", "rebuild", "--release"])),
            ("rebuild".to_owned(), true)
        );
    }

    #[test]
    fn parse_cli_task_only() {
        assert_eq!(parse_cli(&args(&["xtask", "stage"])), ("stage".to_owned(), false));
    }

    #[test]
    fn profile_dir_picks_profile() {
        let base = Path::new("target");
        assert_eq!(profile_dir(base, false), base.join("debug"));
        assert_eq!(profile_dir(base, true), base.join("release"));
        assert_ne!(profile_dir(base, false), profile_dir(base, true));
    }

    #[test]
    fn stage_manifest_has_expected_sources() {
        let items = stage_manifest("elam_rs");
        let item = |name: &str| items.iter().find(|i| i.name == name).unwrap();

        assert!(!item("elam_rs.sys").from_root);
        assert!(!item("ppl_test.exe").from_root);
        assert!(item("elam_rs.pfx").from_root);
        assert!(item("elam_rs.inf").from_root);
        assert!(item("install.ps1").from_root);
        assert!(item("uninstall.ps1").from_root);
        assert!(item("diagnose.ps1").from_root);
    }
}
