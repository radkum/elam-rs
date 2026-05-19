# elam-rs

A minimal ELAM (Early Launch Anti-Malware) driver written in Rust, used as a
**PPL (Protected Process Light) anchor** for an anti-malware / EDR service.

### What this driver does (and what it does not)

ELAM drivers can serve two purposes:

1. **Anti-malware scanning** of boot-start drivers via the boot driver callback.
2. **PPL anchoring** — the certificate embedded in the driver's
   `MicrosoftElamCertificateInfo` resource lets Windows grant `PsProtectedSignerAntimalware`
   protection to a user-mode service signed with the same cert.

**This driver only does #2.** It does not classify, scan, or block any
boot-start driver. Every image is reported as `BdCbClassificationUnknownImage`,
which means the boot loader applies its default policy.

### Why the boot driver callback is still registered

The PPL trust path reads the cert info directly from the driver's embedded
resource — it does not call the boot driver callback. Strictly speaking, the
callback is not needed to *validate* PPL.

However, the Microsoft DDK requires every ELAM driver to call
`IoRegisterBootDriverCallback` from `DriverEntry`. Skipping the registration is
risky:

- Windows may stop treating the image as a real ELAM driver.
- `NtSetInformationProcess(ProcessProtectionInformation)` then fails with
  `STATUS_INVALID_SIGNATURE` and the cause is not obvious.
- The behavior is not contractually guaranteed to stay the same between
  Windows versions.

So we register a minimal, no-op callback. It costs ~10 lines, has no global
state, performs no allocation, and never blocks an image.

### Build

Prerequisites: WDK installed and configured for Rust — see
[windows-drivers-rs](https://github.com/microsoft/windows-drivers-rs/).

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

### Install and run

```
sc create Elam binpath=<path>\elam_rs.sys type=kernel start=boot error=critical group=Early-Launch
```

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
`launchProtected=2`, or (2) being launched by another PPL process. The service
route is the only one that does not already require a PPL parent.

Build and sign the test service with the same `elam_rs.pfx`:

```
cargo xtask ppl-test
```

Install and start (administrator):

```cmd
sc create ElamPplTest binPath= "C:\VSExclude\elam-rs\target\debug\ppl_test.exe" type= own start= demand
sc config ElamPplTest launchProtected= 2
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
  `sc qc ElamPplTest` shows `LAUNCH_PROTECTED : 2`.

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
