# Testing elam-rs + ppl-test

End-to-end walkthrough for verifying that the ELAM driver anchors PPL correctly.

Build happens on the **host**. Installation and testing happen on the **VM**.

> **Use a VM.** Installing a boot driver with `start=boot` on a misconfigured system
> can prevent Windows from starting. Test signing mode also weakens driver security.

---

## Prerequisites

| Where | Requirement |
|---|---|
| Host | WDK + Rust toolchain — `cargo xtask --help` succeeds |
| VM | Windows 10/11 x64, snapshot taken before testing |
| VM | Shared folder or other file transfer to host |
| VM (optional) | Process Hacker 2 / SystemInformer for visual verification |

---

## Step 1 — Enable test signing on the VM (one-time, requires reboot)

On the VM, run as **Administrator**:

```cmd
bcdedit /set testsigning on
shutdown /r /t 0
```

After reboot the desktop shows a "Test Mode" watermark.

Verify:
```cmd
bcdedit | findstr testsigning
```
Expected: `testsigning    Yes`

---

## Step 2 — Build on the host

```cmd
cargo xtask rebuild
cargo xtask ppl-test
```

Both must complete with `signtool` reporting 0 errors.

> `elam_rs.sys` and `ppl_test.exe` must be signed with the **same** `elam_rs.pfx`.
> If you regenerate the cert (`cargo xtask resources`) you must rebuild everything.

---

## Step 3 — Stage artifacts on the host

```cmd
cargo xtask stage
```

This collects the following files into `target\stage\`:

```
elam_rs.sys       driver binary
elam_rs.inf       INF for service registration
ppl_test.exe      PPL test service
install.ps1       installation script (run on VM)
uninstall.ps1     cleanup script (run on VM)
```

---

## Step 4 — Copy `target\stage\` to the VM

Use a shared folder, `scp`, drag-and-drop, or whatever is convenient.
The files must all be in the **same directory** on the VM.

---

## Step 5 — Install on the VM

In the directory where you copied the files, run as **Administrator**:

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

The script:
1. Installs `elam_rs.sys` via INF → copies to `%SystemRoot%\System32\drivers`, registers `Elam` service as `boot` / `Early-Launch`
2. Creates the `ElamPplTest` service with `launchProtected=3`

Verify:
```cmd
sc qc Elam
```
Expected: `START_TYPE : 0  BOOT_START` and `GROUP : Early-Launch`

---

## Step 6 — Reboot the VM

The ELAM driver is a boot-start driver — it only loads on the next boot.

```cmd
shutdown /r /t 0
```

After reboot, verify the driver loaded:
```cmd
sc query Elam
```
Expected: `STATE : 4  RUNNING`

---

## Step 7 — Start the PPL test service

```cmd
sc start ElamPplTest
```

---

## Step 8 — Check the log

```cmd
type C:\Windows\Temp\ppl_test.log
```

**Success output:**
```
[1747384210] PPL test service started, PID=1234
[1747384210] Protection level: 0x00000003 (Antimalware-Light)
[1747384215] still alive
[1747384220] still alive
```

`0x00000003 (Antimalware-Light)` confirms PPL anchoring works.

**Visual check in Process Hacker / SystemInformer:**
find `ppl_test.exe` — the `Protection` column should read `Antimalware (Light)`.

---

## Failure signals

| Symptom | Cause | Fix |
|---|---|---|
| `sc start` → 1297 `ERROR_PRIVILEGE_NOT_HELD` | Driver not recognized as ELAM, or ppl_test cert doesn't chain to the embedded hash | Confirm `sc query Elam` shows RUNNING; rebuild everything with a fresh cert |
| `sc start` → 577 `ERROR_INVALID_IMAGE_HASH` and CodeIntegrity event 3033 "did not meet the Custom 3 / Antimalware signing level" | The driver's `MicrosoftElamCertificateInfo` resource is missing or misformatted, so the cert is not registered as an ELAM anchor | Make sure `generate_cert.ps1` emits the `MicrosoftElamCertificateInfo MSElamCertInfoID` form, then `cargo xtask rebuild`, reinstall and reboot. Also confirm the cert is in Root/TrustedPublisher and all binaries were signed with the same `.pfx`. |
| Log shows `0xFFFFFFFE (None)` | `launchProtected` not applied | Verify `(Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Services\ElamPplTest').LaunchProtected` is `3`; re-run `install.ps1` (it sets this DWORD) and reboot |
| Log shows `<query failed>` | `GetProcessInformation` failed | Same as above |
| `sc query Elam` → STOPPED after reboot | Driver failed to load at boot | Check System event log → source `Service Control Manager` |

---

## Known limitations

- **Image classification is inert under test signing.** `DriverEntry` attempts
  `IoRegisterBootDriverCallback`, but Windows refuses registration for a
  self-signed (test-signed) ELAM driver. The driver stays resident so it can
  still anchor PPL, but `KNOWN_GOOD_HASHES` / `KNOWN_BAD_HASHES` are never
  consulted. The end-to-end test above therefore verifies **PPL only**.
  Exercising classification requires an ELAM driver signed with a real
  Microsoft ELAM certificate.
- `ppl_test.exe` is a benign logger: `0x00000003 (Antimalware-Light)` proves the
  process was launched protected by the ELAM anchor, not that any scanning or
  blocking happened.

---

## Cleanup

On the VM, as **Administrator**:

```powershell
powershell -ExecutionPolicy Bypass -File .\uninstall.ps1
shutdown /r /t 0
```

The script uninstalls the ELAM driver via INF, removes the certificate and the
log, clears `LaunchProtected`, and tries to stop/delete `ElamPplTest`.

A **running protected service cannot be stopped or deleted, even by an
administrator** — that is the point of PPL. If `ElamPplTest` is still registered
after this run, reboot (it is demand-start, so it will not restart) and run
`uninstall.ps1` again; the second run deletes the service record immediately now
that `LaunchProtected` is cleared (no extra reboot needed for it — the reboot
between the two runs also completes driver removal).

To turn off test signing after everything is removed:

```cmd
bcdedit /set testsigning off
shutdown /r /t 0
```
