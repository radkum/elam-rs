#![no_std]
#![allow(non_snake_case)]

#[cfg(not(test))]
extern crate wdk_panic;

#[cfg(not(test))]
use wdk_alloc::WdkAllocator;

#[cfg(not(test))]
#[global_allocator]
static GLOBAL_ALLOCATOR: WdkAllocator = WdkAllocator;

use wdk_sys::{DRIVER_OBJECT, NTSTATUS, PCUNICODE_STRING, PDRIVER_OBJECT, STATUS_SUCCESS};

// IoRegisterBootDriverCallback is intentionally not called.
//
// In test-signing mode Windows refuses the ELAM callback registration for
// drivers not signed with a Microsoft ELAM certificate, causing DriverEntry
// to fail and the driver to unload — which removes the in-memory image that
// CI needs to read MicrosoftElamCertificateInfo from for PPL trust.
//
// The PPL trust path reads the cert hash from the *loaded* driver image;
// the boot driver callback is for boot-driver classification, not PPL.
// We skip the registration so the driver stays resident and CI can access
// the embedded resource.
#[link_section = "INIT"]
#[export_name = "DriverEntry"]
pub unsafe extern "system" fn driver_entry(
    driver_object: &mut DRIVER_OBJECT,
    _registry_path: PCUNICODE_STRING,
) -> NTSTATUS {
    driver_object.DriverUnload = Some(driver_unload);
    STATUS_SUCCESS
}

extern "C" fn driver_unload(_driver: PDRIVER_OBJECT) {}
