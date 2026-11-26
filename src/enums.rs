//! Boot-start image classification tables.
//!
//! These tables are generated at build time from `hashes/known_good.txt` and
//! `hashes/known_bad.txt` (see `build.rs`). Each line is a SHA-256 digest as 64
//! hex characters; blank lines and `#` comments are ignored.
//!
//! An image whose `BDCB_IMAGE_INFORMATION::ImageHash` matches an entry in
//! `KNOWN_BAD_HASHES` is reported as `KnownBadImage`, one matching
//! `KNOWN_GOOD_HASHES` as `KnownGoodImage`, and everything else as
//! `UnknownImage` (the boot loader's default policy applies).
//!
//! Both files are empty by default.

include!(concat!(env!("OUT_DIR"), "/known_hashes.rs"));
