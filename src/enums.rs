#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

#[repr(C)]
pub struct BDCB_CALLBACK_TYPE_ENUM(pub i32);

impl BDCB_CALLBACK_TYPE_ENUM {
    pub const BdCbInitializeImage: i32 = 0x00000001;
    pub const BdCbStatusUpdate: i32 = 0x00000000;
}

#[repr(C)]
pub struct BDCB_STATUS_UPDATE_TYPE_ENUM(pub i32);

impl BDCB_STATUS_UPDATE_TYPE_ENUM {
    pub const BdCbStatusPrepareForDependencyLoad: i32 = 0x00000000;
    pub const BdCbStatusPrepareForDriverLoad: i32 = 0x00000001;
    pub const BdCbStatusPrepareForUnload: i32 = 0x00000002;
}

pub(crate) const BAD_IMAGES: [&str; 2] = ["Malicious1", "Malicious2"];
pub(crate) const GOOD_IMAGES: [&str; 2] = ["Good1", "Good2"];
