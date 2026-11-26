#![no_std]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

#[cfg(not(test))]
extern crate wdk_panic;

#[cfg(not(test))]
use wdk_alloc::WDKAllocator;

#[cfg(not(test))]
#[global_allocator]
static GLOBAL_ALLOCATOR: WDKAllocator = WDKAllocator;

extern crate alloc;

use alloc::string::String;
use core::{
    ffi::c_void,
    ptr::null_mut,
    sync::atomic::{AtomicI32, AtomicPtr, Ordering},
};

use elam_core::{classify_by_hash, decode_utf16_lossy, format_hex, Classification, SHA256_LEN};
use wdk::{print, println};
use wdk_sys::{
    ntddk::{IoRegisterBootDriverCallback, IoUnregisterBootDriverCallback},
    _BDCB_CALLBACK_TYPE::{BdCbInitializeImage, BdCbStatusUpdate},
    _BDCB_CLASSIFICATION::{
        BdCbClassificationKnownBadImage, BdCbClassificationKnownGoodImage,
        BdCbClassificationUnknownImage,
    },
    _BDCB_STATUS_UPDATE_TYPE::{
        BdCbStatusPrepareForDependencyLoad, BdCbStatusPrepareForDriverLoad,
        BdCbStatusPrepareForUnload,
    },
    BDCB_CALLBACK_TYPE, BDCB_IMAGEFLAGS_FAILED_CODE_INTEGRITY, BDCB_IMAGE_INFORMATION,
    BDCB_STATUS_UPDATE_TYPE, DRIVER_OBJECT, NTSTATUS, PBDCB_IMAGE_INFORMATION,
    PBDCB_STATUS_UPDATE_CONTEXT, PCUNICODE_STRING, PDRIVER_OBJECT, PVOID, STATUS_SUCCESS,
    UNICODE_STRING,
};

mod enums;
use crate::enums::{KNOWN_BAD_HASHES, KNOWN_GOOD_HASHES};

static BOOT_DRIVER_CALLBACK_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static CURRENT_BCD_CALLBACK_CONTEXT_TYPE: AtomicI32 =
    AtomicI32::new(BdCbStatusPrepareForDependencyLoad);

#[link_section = "INIT"]
#[export_name = "DriverEntry"] // WDF expects a symbol with the name DriverEntry
pub unsafe extern "system" fn driver_entry(
    driver_object: &mut DRIVER_OBJECT,
    _registry_path: PCUNICODE_STRING,
) -> NTSTATUS {
    println!("Elam - driver_entry START");

    driver_object.DriverUnload = Some(ElamDriverUnload);

    let handle = IoRegisterBootDriverCallback(Some(ElamBootDriverCallback), null_mut());
    BOOT_DRIVER_CALLBACK_HANDLE.store(handle, Ordering::Release);

    if handle.is_null() {
        // Best-effort: under test signing Windows refuses ELAM callback
        // registration for self-signed drivers. Do not fail DriverEntry — keep
        // the image resident so its embedded certificate can still anchor PPL.
        // Classification simply does not run in that case.
        println!("Elam - boot driver callback unavailable; classification disabled");
    } else {
        println!("Elam - boot driver callback registered");
    }

    println!("Elam - driver_entry END");
    STATUS_SUCCESS
}

/*************************************************************************
                    Dispatch routines.
*************************************************************************/
unsafe extern "C" fn ElamDriverUnload(_driver: PDRIVER_OBJECT) {
    let handle = BOOT_DRIVER_CALLBACK_HANDLE.swap(null_mut(), Ordering::AcqRel);
    if !handle.is_null() {
        IoUnregisterBootDriverCallback(handle);
    }

    println!("ElamDriverUnload: BYE");
}

unsafe extern "C" fn ElamBootDriverCallback(
    callback_context: PVOID,
    classification: BDCB_CALLBACK_TYPE,
    image_information: PBDCB_IMAGE_INFORMATION,
) {
    // IoRegisterBootDriverCallback was called with a null context.
    if !callback_context.is_null() {
        println!("Elam-rs: unexpected non-null callback context");
    }

    match classification {
        BdCbStatusUpdate => {
            let update_status = image_information as PBDCB_STATUS_UPDATE_CONTEXT;
            if !update_status.is_null() {
                ElamProcessStatusUpdate((*update_status).StatusType);
            }
        },
        BdCbInitializeImage => ElamProcessInitializeImage(image_information),
        _ => {},
    }
}

fn ElamProcessStatusUpdate(status: BDCB_STATUS_UPDATE_TYPE) {
    match status {
        BdCbStatusPrepareForDependencyLoad => {
            println!(
                "Elam-rs reports that Boot Start driver dependencies are being \
                 initialized.\r\n\r\n"
            )
        },
        BdCbStatusPrepareForDriverLoad => {
            println!("Elam-rs reports that Boot Start drivers are about to be initialized.\r\n\r\n")
        },
        BdCbStatusPrepareForUnload => println!(
            "Elam-rs reports that all Boot Start drivers have been initialized and that Elam-rs \
             is about to be unloaded\r\n\r\n"
        ),
        _ => println!("Elam-rs reports an unknown status type.\r\n\r\n"),
    }

    CURRENT_BCD_CALLBACK_CONTEXT_TYPE.store(status, Ordering::Release);
}

unsafe fn ElamProcessInitializeImage(image_information: PBDCB_IMAGE_INFORMATION) {
    if image_information.is_null() {
        println!("Elam-rs: null image information");
        return;
    }

    match CURRENT_BCD_CALLBACK_CONTEXT_TYPE.load(Ordering::Acquire) {
        BdCbStatusPrepareForDependencyLoad => {
            println!("Elam-rs reports the following dependency is about to be initialized:\r\n")
        },
        BdCbStatusPrepareForDriverLoad => {
            println!(
                "Elam-rs reports the following Boot Start driver is about to be initialized:\r\n"
            )
        },
        _ => println!("Elam-rs reports an invalid status type for image initialization:\r\n"),
    }

    let image = &mut *image_information;

    match ElamDecodeUnicodeString(&image.ImageName) {
        Some(name) => println!("Elam-rs:    Image name \"{name}\"\r\n"),
        None => println!("Elam-rs:    Image name <unnamed>\r\n"),
    }

    if let Some(path) = ElamDecodeUnicodeString(&image.RegistryPath) {
        println!("Elam-rs:    Registry path \"{path}\"\r\n");
    }

    // Did this image fail Code Integrity checks?
    if (image.ImageFlags & BDCB_IMAGEFLAGS_FAILED_CODE_INTEGRITY) != 0 {
        println!(
            "Elam-rs:    FAILED Code Integrity checks but boot policy allowed it to be loaded.\r\n"
        );
    }

    if !image.ImageHash.is_null() && image.ImageHashLength != 0 {
        println!("Elam-rs:    Image hash algorithm = 0x{:08x}.\r\n", image.ImageHashAlgorithm);
        println!("Elam-rs:    Image hash:\r\n");

        let hash = core::slice::from_raw_parts(
            image.ImageHash as *const u8,
            image.ImageHashLength as usize,
        );
        print!("{}", format_hex(hash, 16));
        println!("\r\n");
    }

    if let Some(publisher) = ElamDecodeUnicodeString(&image.CertificatePublisher) {
        println!("Elam-rs:    Image is signed by \"{publisher}\".\r\n");

        if let Some(issuer) = ElamDecodeUnicodeString(&image.CertificateIssuer) {
            println!("Elam-rs:    Certificate issued by \"{issuer}\".\r\n");
        }

        if !image.CertificateThumbprint.is_null() && image.CertificateThumbprintLength != 0 {
            println!(
                "Elam-rs:    Certificate thumb print algorithm = 0x{:08x}.\r\n",
                image.ThumbprintHashAlgorithm
            );
            println!("Elam-rs:    Certificate thumb print:\r\n");

            let thumbprint = core::slice::from_raw_parts(
                image.CertificateThumbprint as *const u8,
                image.CertificateThumbprintLength as usize,
            );
            print!("{}", format_hex(thumbprint, 16));
            println!("\r\n");
        }
    } else {
        println!("Elam-rs:    Not signed.\r\n");
    }

    image.Classification = ElamClassifyImage(image);
}

fn ElamClassifyImage(image: &BDCB_IMAGE_INFORMATION) -> i32 {
    if image.ImageHash.is_null() || image.ImageHashLength as usize != SHA256_LEN {
        return BdCbClassificationUnknownImage;
    }

    let hash = unsafe {
        core::slice::from_raw_parts(image.ImageHash as *const u8, image.ImageHashLength as usize)
    };

    match classify_by_hash(image.ImageHashAlgorithm, hash, &KNOWN_GOOD_HASHES, &KNOWN_BAD_HASHES) {
        Classification::KnownGood => BdCbClassificationKnownGoodImage,
        Classification::KnownBad => BdCbClassificationKnownBadImage,
        Classification::Unknown => BdCbClassificationUnknownImage,
    }
}

/// Safely decode a `UNICODE_STRING`, returning `None` for null/empty buffers.
unsafe fn ElamDecodeUnicodeString(value: &UNICODE_STRING) -> Option<String> {
    if value.Buffer.is_null() || value.Length == 0 {
        return None;
    }

    let len = value.Length as usize / core::mem::size_of::<u16>();
    let wide = core::slice::from_raw_parts(value.Buffer as *const u16, len);
    decode_utf16_lossy(Some(wide))
}
