# elam-rs

A minimal ELAM (Early Launch Anti-Malware) driver written in Rust, used as a
**PPL (Protected Process Light) anchor** for an anti-malware / EDR service.

### What this driver does

ELAM drivers can serve two purposes:

1. **Anti-malware scanning** of boot-start drivers via the boot driver callback —
   this driver classifies each image by its SHA-256 hash against the
   `KNOWN_GOOD_HASHES` / `KNOWN_BAD_HASHES` tables (generated from
   `hashes/known_good.txt` and `hashes/known_bad.txt`), reporting
   `KnownGoodImage`, `KnownBadImage`, or `UnknownImage`. Images not in either
   table get the boot loader's default policy.
2. **PPL anchoring** — the certificate embedded in the driver's
   `MicrosoftElamCertificateInfo` resource lets Windows grant
   `PsProtectedSignerAntimalware` protection to a user-mode service signed with
   the same cert.

Both are enabled. The mechanisms are independent: classification only takes
effect when the boot driver callback is available, while PPL anchoring relies
solely on the embedded certificate hash.

### Boot driver callback registration is best-effort

`DriverEntry` calls `IoRegisterBootDriverCallback`. On a properly signed ELAM
driver this succeeds and classification runs.

Under **test signing** with a self-signed certificate, Windows refuses the
registration. This is not fatal: `DriverEntry` logs the failure and returns
`STATUS_SUCCESS` so the driver stays resident. That matters because the PPL
trust path reads the certificate hash from the *loaded* driver image, not from
the callback. The driver therefore still anchors PPL but performs no
classification until it is signed with a real Microsoft ELAM certificate.

### Classifying images

`hashes/known_good.txt` and `hashes/known_bad.txt` are read by `build.rs` and
turned into the `KNOWN_GOOD_HASHES` / `KNOWN_BAD_HASHES` tables. Write one
64-hex-character SHA-256 digest per line; blank lines and `#` comments are
allowed. A malformed line fails the build. Rebuild after editing.

### Build

Prerequisites: WDK installed and configured for Rust — see
[windows-drivers-rs](https://github.com/microsoft/windows-drivers-rs/). The
`wdk-sys` bindings are generated with bindgen at build time, so LLVM/libclang
must be installed and discoverable (on `PATH` or via `LIBCLANG_PATH`).

The build pipeline is driven by an `xtask` crate (replaces the old
`Makefile.toml`):

```
cargo xtask                    # default: resources -> build -> rename -> sign
cargo xtask --release          # release build of the same pipeline
cargo xtask build              # only compile (no cert, no rename, no sign)
cargo xtask rebuild --release  # clean + full pipeline
cargo xtask --help             # list all tasks
```

The pipeline:

1. **resources** — `generate_cert.ps1` creates a self-signed cert
   (`elam_rs.pfx`) and writes its TBS hash into `elam_rs.rc`. The hash is what
   Windows binds to PPL.
2. **build** — `cargo build`, producing `elam_rs.dll`.
3. **rename** — `.dll` → `.sys`.
4. **sign** — `signtool.exe sign` with `elam_rs.pfx`.

`signtool.exe` is located automatically (PATH, then the newest installed
Windows SDK under `C:\Program Files (x86)\Windows Kits\10\bin`).

Output: `target/{debug,release}/elam_rs.sys`.

Pure logic (hash comparison, classification, hex formatting) lives in the
`elam-core` crate and is covered by host unit tests:

```
cargo test -p elam-core
```

CI runs formatting, those tests and clippy. The driver itself needs the WDK and
is built locally with `cargo xtask`.

### Install and run

```
sc create Elam binpath=<path>\elam_rs.sys type=kernel start=boot error=normal group=Early-Launch
```

`error=normal` (not `critical`) is deliberate: on a test VM a failed boot-time
driver should not make the system unbootable. A production ELAM driver would use
`error=critical`.

The host VM must allow test-signed drivers:

```
bcdedit /set testsigning on
```

(administrator, reboot required).

The user-mode service that wants PPL must be signed with the **same**
certificate (`elam_rs.pfx`), and request protection via
`NtSetInformationProcess` with `PsProtectedSignerAntimalware`.

### Testing the PPL anchor

A minimal test service is included in `ppl-test/` to verify that the driver
actually anchors PPL. It is a Windows service that, once started, logs its own
protection level to `C:\Windows\Temp\ppl_test.log` and keeps running until
stopped. There is no console interaction — PPL processes have no access to an
interactive desktop, so `println!` would go nowhere.

Why a service: a user-mode process cannot elevate itself to PPL. The two
sanctioned paths are (1) being launched by `services.exe` with
`launchProtected=3`, or (2) being launched by another PPL process. The service
route is the only one that does not already require a PPL parent.

Build and sign the test service with the same `elam_rs.pfx`:

```
cargo xtask ppl-test
```

Install and start (administrator):

```cmd
sc create ElamPplTest binPath= "C:\VSExclude\elam-rs\target\debug\ppl_test.exe" type= own start= demand
sc config ElamPplTest launchProtected= 3
sc start ElamPplTest
```

(spaces after `=` are required by `sc.exe`)

Check the log:

```cmd
type C:\Windows\Temp\ppl_test.log
```

Expected output:

```
[<timestamp>] PPL test service started, PID=<pid>
[<timestamp>] Protection level: 0x00000003 (Antimalware-Light)
[<timestamp>] still alive
...
```

Cross-check in Process Hacker / SystemInformer: the `Protection` column should
read `Antimalware (Light)` for `ppl_test.exe`.

**Failure signals:**

- `sc start` returns `1297` (`ERROR_PRIVILEGE_NOT_HELD`) — the driver was not
  recognized as ELAM, or the test binary's certificate does not chain to the
  one embedded in `elam_rs.rc`.
- `sc start` returns `577` (`ERROR_INVALID_IMAGE_HASH`) — signature mismatch;
  regenerate the cert with `cargo xtask resources` and rebuild everything so
  the `.rc` and the signing key stay in sync.
- The service starts but the log shows `Protection level: 0xFFFFFFFE (None)` —
  the binary loaded but `launchProtected` did not take effect; verify
  `sc qc ElamPplTest` shows `LAUNCH_PROTECTED : 3`.

Cleanup:

```cmd
sc stop ElamPplTest
sc delete ElamPplTest
```

### References

- [Protecting Anti-Malware Services](https://learn.microsoft.com/en-us/windows/win32/services/protecting-anti-malware-services-)
- [windows-drivers-rs](https://github.com/microsoft/windows-drivers-rs/)
- [Microsoft ELAM sample](https://github.com/microsoft/Windows-driver-samples/blob/main/security/elam/elamsample.c)
- [PPLRunner](https://github.com/pathtofile/PPLRunner/tree/main/ppl_runner)
- [7eRoM/elam](https://github.com/7eRoM/elam/tree/master)
