#![no_std]
#![allow(non_snake_case)]
#![allow(static_mut_refs)]

extern crate alloc;

#[cfg(not(test))]
extern crate wdk_panic;

use alloc::string::String;

#[cfg(not(test))]
use wdk_alloc::WdkAllocator;

#[cfg(not(test))]
#[global_allocator]
static GLOBAL_ALLOCATOR: WdkAllocator = WdkAllocator;

use core::ptr::null_mut;

use wdk::{print, println};
use wdk_sys::{
    ntddk::{IoRegisterBootDriverCallback, IoUnregisterBootDriverCallback},
    BDCB_CALLBACK_TYPE, BDCB_IMAGEFLAGS_FAILED_CODE_INTEGRITY, BDCB_IMAGE_INFORMATION,
    BDCB_STATUS_UPDATE_TYPE, DRIVER_OBJECT, NTSTATUS, PBDCB_IMAGE_INFORMATION,
    PBDCB_STATUS_UPDATE_CONTEXT, PCUNICODE_STRING, PDRIVER_OBJECT, PUCHAR, PVOID, STATUS_SUCCESS,
    STATUS_UNSUCCESSFUL, ULONG, UNICODE_STRING, _BDCB_CLASSIFICATION as BDCB_CLASSIFICATION,
    _BDCB_STATUS_UPDATE_TYPE as BDCB_STATUS_UPDATE_TYPE_ENUM,
    _BDCB_STATUS_UPDATE_TYPE::BdCbStatusPrepareForDependencyLoad,
};
mod enums;
use crate::enums::{BAD_IMAGES, BDCB_CALLBACK_TYPE_ENUM, GOOD_IMAGES};

static mut G_IO_REGISTER_BOOT_DRIVER_CALLBACK_HANDLE: PVOID = null_mut();
static mut G_CURRENT_BCD_CALLBACK_CONTEXT_TYPE: BDCB_STATUS_UPDATE_TYPE =
    BdCbStatusPrepareForDependencyLoad;

#[link_section = "INIT"]
#[export_name = "DriverEntry"] // WDF expects a symbol with the name DriverEntry
pub unsafe extern "system" fn driver_entry(
    driver_object: &mut DRIVER_OBJECT,
    _registry_path: PCUNICODE_STRING,
) -> NTSTATUS {
    println!("Elam - driver_entry START");

    driver_object.DriverUnload = Some(ElamDriverUnload);

    G_IO_REGISTER_BOOT_DRIVER_CALLBACK_HANDLE =
        IoRegisterBootDriverCallback(Some(ElamBootDriverCallback), null_mut());

    if G_IO_REGISTER_BOOT_DRIVER_CALLBACK_HANDLE.is_null() {
        return STATUS_UNSUCCESSFUL;
    }
    println!("Elam - driver_entry END");
    STATUS_SUCCESS
}

/*************************************************************************
                    Dispatch  routines.
*************************************************************************/
extern "C" fn ElamDriverUnload(_driver: PDRIVER_OBJECT) {
    unsafe {
        if !G_IO_REGISTER_BOOT_DRIVER_CALLBACK_HANDLE.is_null() {
            IoUnregisterBootDriverCallback(G_IO_REGISTER_BOOT_DRIVER_CALLBACK_HANDLE);
            G_IO_REGISTER_BOOT_DRIVER_CALLBACK_HANDLE = null_mut();
        }
    }

    println!("ElamDriverUnload: BYE");
}

extern "C" fn ElamBootDriverCallback(
    CallbackContext: PVOID,
    Classification: BDCB_CALLBACK_TYPE,
    ImageInformation: PBDCB_IMAGE_INFORMATION,
) {
    // IoRegisterBootDriverCallback was called with a null context. Ensure that is passed here.
    println!("ElamBootDriverCallback");
    if !CallbackContext.is_null() {
        println!("Elam-rs has been passed an unexpected callback context");
    }

    match Classification {
        BDCB_CALLBACK_TYPE_ENUM::BdCbStatusUpdate => {
            let update_status = ImageInformation as PBDCB_STATUS_UPDATE_CONTEXT;
            unsafe {
                ElamProcessStatusUpdate((*update_status).StatusType);
            }
        },
        BDCB_CALLBACK_TYPE_ENUM::BdCbInitializeImage => unsafe {
            ElamProcessInitializeImage(ImageInformation);
        },
        _ => {},
    }
}

extern "C" fn ElamProcessStatusUpdate(StatusType: BDCB_STATUS_UPDATE_TYPE) {
    match StatusType {
        BDCB_STATUS_UPDATE_TYPE_ENUM::BdCbStatusPrepareForDependencyLoad => println!(
            "Elam-rs reports that Boot Start driver dependencies are being initialized.\r\n\r\n"
        ),
        BDCB_STATUS_UPDATE_TYPE_ENUM::BdCbStatusPrepareForDriverLoad => {
            println!("Elam-rs reports that Boot Start drivers are about to be initialized.\r\n\r\n")
        },
        BDCB_STATUS_UPDATE_TYPE_ENUM::BdCbStatusPrepareForUnload => println!(
            "Elam-rs reports that all Boot Start drivers have been initialized and that Elam-rs \
             is about to be unloaded\r\n\r\n"
        ),
        _ => println!("Elam-rs reports an unknown status type.\r\n\r\n"),
    }
    unsafe {
        G_CURRENT_BCD_CALLBACK_CONTEXT_TYPE = StatusType;
    }
}

unsafe extern "C" fn ElamProcessInitializeImage(ImageInformation: PBDCB_IMAGE_INFORMATION) {
    // Is this a dependency or a boot start driver?
    if G_CURRENT_BCD_CALLBACK_CONTEXT_TYPE
        == BDCB_STATUS_UPDATE_TYPE_ENUM::BdCbStatusPrepareForDependencyLoad
    {
        println!("Elam-rs reports the following dependency is about to be initialized:\r\n");
    } else if G_CURRENT_BCD_CALLBACK_CONTEXT_TYPE
        == BDCB_STATUS_UPDATE_TYPE_ENUM::BdCbStatusPrepareForDriverLoad
    {
        println!("Elam-rs reports the following Boot Start driver is about to be initialized:\r\n");
    } else {
        println!("Elam-rs reports an invalid status type for image initialization:\r\n");
    }

    let ImageInformation = unsafe { &mut *ImageInformation };

    // Display the image name and any associated registry path.
    println!("Elam-rs:    Image name \"{:p}\"\r\n", &ImageInformation.ImageName);

    if ImageInformation.RegistryPath.Buffer.is_null() {
        println!("Elam-rs:    Registry path \"{:p}\"\r\n", &ImageInformation.RegistryPath);
    }

    // Did this image fail Code Integrity checks?
    if (ImageInformation.ImageFlags & BDCB_IMAGEFLAGS_FAILED_CODE_INTEGRITY) != 0 {
        println!(
            "Elam-rs:    FAILED Code Integrity checks but boot policy allowed it to be loaded.\r\n"
        );
    }

    // Display the image's hash.
    if !ImageInformation.ImageHash.is_null() && ImageInformation.ImageHashLength != 0 {
        println!(
            "Elam-rs:    Image hash algorithm = 0x{:08x}.\r\n",
            ImageInformation.ImageHashAlgorithm
        );
        println!("Elam-rs:    Image hash:");

        ElamPrintHex(ImageInformation.ImageHash, ImageInformation.ImageHashLength);
    }

    // Display who signed the image (if at all).
    if !ImageInformation.CertificatePublisher.Buffer.is_null() {
        println!(
            "Elam-rs:    Image is signed by \"{:p}\".\r\n",
            &ImageInformation.CertificatePublisher
        );

        if !ImageInformation.CertificateIssuer.Buffer.is_null() {
            println!(
                "Elam-rs:    Certificate issued by \"{:p}\".\r\n",
                &ImageInformation.CertificateIssuer
            );
        }

        if !ImageInformation.CertificateThumbprint.is_null()
            && ImageInformation.CertificateThumbprintLength != 0
        {
            println!(
                "Elam-rs:    Certificate thumb print algorithm = 0x{:08x}.\r\n",
                ImageInformation.ThumbprintHashAlgorithm
            );
            println!("Elam-rs:    Certificate thumb print:");

            ElamPrintHex(
                ImageInformation.CertificateThumbprint,
                ImageInformation.CertificateThumbprintLength,
            );
        }
    } else {
        println!("Elam-rs:    Not signed.\r\n");
    }

    ImageInformation.Classification = ElamClassifyImage(ImageInformation);
}

extern "C" fn ElamClassifyImage(image_info: &BDCB_IMAGE_INFORMATION) -> i32 {
    let image_name = unicode_to_rust_str(image_info.ImageName);
    if BAD_IMAGES.contains(&image_name.as_str()) {
        BDCB_CLASSIFICATION::BdCbClassificationKnownBadImage
    } else if GOOD_IMAGES.contains(&image_name.as_str()) {
        BDCB_CLASSIFICATION::BdCbClassificationKnownGoodImage
    } else {
        BDCB_CLASSIFICATION::BdCbClassificationUnknownImage
    }
}

extern "C" fn ElamPrintHex(Data: PVOID, DataSize: ULONG) {
    for i in 0..DataSize {
        let bytes: PUCHAR = Data as PUCHAR;
        if (i % 15) == 0 {
            println!("\r\nElam-rs:    ");
        }
        unsafe {
            print!("{:02x} ", *bytes.offset(i as isize));
        }
    }

    println!("\r\n");
}

fn unicode_to_rust_str(unicode_str: UNICODE_STRING) -> String {
    String::from_utf16_lossy(unsafe {
        core::slice::from_raw_parts(
            unicode_str.Buffer,
            unicode_str.Length as usize / core::mem::size_of_val(&(*unicode_str.Buffer)),
        )
    })
}
