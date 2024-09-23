# elam-rs

The simple ELAM driver written in Rust.

### Description
Elam (Early Launch Antimalware) driver deliver support for anti-malware software on Windows OS.
It allows to things:
- start before third-party components and control initialization of potentially malicious drivers
- store certificate used to sign antimalware service and run in as PPL (process protected light) process

### Getting Started
1. At the beginning you need WDK and other stuff to build rust drivers. Let's look at: https://github.com/microsoft/windows-drivers-rs/
2. Next step is to install cargo make: https://github.com/sagiegurari/cargo-make
3. Use generate_cert.ps1 script to generate certificate necessary to signing, and resources to include by ELAM. Use:
   `cargo make resources`
4. Build a binaries using: `cargo make compile`
5. Rename elam_rs.dll to elam_rs.sys: `cargo make rename`
6. At the end sign binaries. Use: `cargo make sign`

PS: You can use `cargo make` to invoke these four last steps

A signed `elam_rs.sys` is produced into `target/debug` directory

### How to run
Install driver:
`sc create Elam binpath=<path>\elam_rs.sys type=kernel start=boot error=critical group=Early-Launch`

IMPORTANT!!!
<br>Virtual machine must work in sign testing mode. To set this option run `bcdedit /set testsigning on`
with administrator privileges and reboot your PC

### Links:
- https://learn.microsoft.com/en-us/windows/win32/services/protecting-anti-malware-services-
- https://github.com/microsoft/windows-drivers-rs/
- https://github.com/microsoft/Windows-driver-samples/blob/main/security/elam/elamsample.c
- https://github.com/pathtofile/PPLRunner/tree/main/ppl_runner
- https://github.com/7eRoM/elam/tree/master
