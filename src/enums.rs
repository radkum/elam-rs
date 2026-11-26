//! Boot-start image classification tables.
//!
//! Images are classified by their SHA-256 digest, as reported by the boot
//! driver callback in `BDCB_IMAGE_INFORMATION::ImageHash`. Add an entry as a
//! `[u8; SHA256_LEN]` (32-byte) array. Example:
//!
//! ```ignore
//! pub(crate) const KNOWN_BAD_HASHES: [[u8; SHA256_LEN]; 1] = [[
//!     0xde, 0xad, 0xbe, 0xef, /* ... 32 bytes total ... */
//! ]];
//! ```
//!
//! Both tables are empty by default, which means every image is reported as
//! `UnknownImage` and the boot loader keeps its default policy.

use elam_core::SHA256_LEN;

/// SHA-256 hashes of boot-start images that are known to be safe.
pub(crate) const KNOWN_GOOD_HASHES: [[u8; SHA256_LEN]; 0] = [];

/// SHA-256 hashes of boot-start images that are known to be malicious.
pub(crate) const KNOWN_BAD_HASHES: [[u8; SHA256_LEN]; 0] = [];
