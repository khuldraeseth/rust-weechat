//! A safe and high level API to access HData tables

use crate::{Buffer, LossyCString, Weechat};
use chrono::{DateTime, NaiveDateTime, Utc};
use std::borrow::Cow;
use std::convert::TryInto;
use std::ffi::{c_void, CStr};
use weechat_sys::{t_hdata, t_weechat_plugin};

/// The HData object represents a table of variables associated with an object.
///
/// An HData object can be created from any Weechat type that implements [`HasHData`] using the
/// [`get_hdata`](HasHData::get_hdata) function and the name of the hdata table you want to access.
pub struct HData {
    weechat_ptr: *mut t_weechat_plugin,
    object: *mut c_void,
    ptr: *mut t_hdata,
}

impl HData {
    /// Retrieve the value of a variable in a hdata.
    pub fn get_var<T: HDataType>(&self, name: &str) -> Option<T> {
        let weechat = Weechat::from_ptr(self.weechat_ptr);

        HDataType::hdata_value(self, name)
    }
}

/// A trait for types that have hdata.
pub trait HasHData {
    /// Retrieve a hdata table tied to this object.
    fn get_hdata(&self, name: &str) -> Option<HData>;
}

impl HasHData for Buffer {
    fn get_hdata(&self, name: &str) -> Option<HData> {
        let hdata_get =
            Weechat::from_ptr(self.weechat).get().hdata_get.unwrap();

        let name = LossyCString::new(name);

        unsafe {
            let hdata = hdata_get(self.weechat, name.as_ptr());
            if hdata.is_null() {
                None
            } else {
                Some(HData {
                    weechat_ptr: self.weechat,
                    object: self.ptr as *mut _,
                    ptr: hdata,
                })
            }
        }
    }
}

/// A trait for types of hdata values.
pub trait HDataType: Sized {
    /// Retrieve the value of a hdata variable by name.
    fn hdata_value(hdata: &HData, name: &str) -> Option<Self>;
}

impl HDataType for Cow<'_, str> {
    fn hdata_value(hdata: &HData, name: &str) -> Option<Self> {
        let weechat = Weechat::from_ptr(hdata.weechat_ptr);
        let hdata_string = weechat.get().hdata_string.unwrap();
        let hdata_get_var_type = weechat.get().hdata_get_var_type.unwrap();

        let name = LossyCString::new(name);

        unsafe {
            if hdata_get_var_type(hdata.ptr, name.as_ptr())
                != weechat_sys::WEECHAT_HDATA_STRING as i32
            {
                return None;
            }

            let ret = hdata_string(hdata.ptr, hdata.object, name.as_ptr());
            if ret.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ret).to_string_lossy())
            }
        }
    }
}

impl HDataType for String {
    fn hdata_value(hdata: &HData, name: &str) -> Option<Self> {
        HDataType::hdata_value(hdata, name).map(Cow::into_owned)
    }
}

impl HDataType for char {
    fn hdata_value(hdata: &HData, name: &str) -> Option<Self> {
        let weechat = Weechat::from_ptr(hdata.weechat_ptr);
        let hdata_char = weechat.get().hdata_char.unwrap();
        let hdata_get_var_type = weechat.get().hdata_get_var_type.unwrap();

        let name = LossyCString::new(name);

        unsafe {
            if hdata_get_var_type(hdata.ptr, name.as_ptr())
                != weechat_sys::WEECHAT_HDATA_CHAR as i32
            {
                return None;
            }

            let c_char = hdata_char(hdata.ptr, hdata.object, name.as_ptr());
            c_char.try_into().map(|ch: u8| ch as char).ok()
        }
    }
}

impl HDataType for i64 {
    fn hdata_value(hdata: &HData, name: &str) -> Option<Self> {
        let weechat = Weechat::from_ptr(hdata.weechat_ptr);
        let hdata_long = weechat.get().hdata_long.unwrap();
        let hdata_get_var_type = weechat.get().hdata_get_var_type.unwrap();

        let name = LossyCString::new(name);

        unsafe {
            if hdata_get_var_type(hdata.ptr, name.as_ptr())
                != weechat_sys::WEECHAT_HDATA_LONG as i32
            {
                return None;
            }

            Some(hdata_long(hdata.ptr, hdata.object, name.as_ptr()))
        }
    }
}

impl HDataType for i32 {
    fn hdata_value(hdata: &HData, name: &str) -> Option<Self> {
        let weechat = Weechat::from_ptr(hdata.weechat_ptr);
        let hdata_integer = weechat.get().hdata_integer.unwrap();
        let hdata_get_var_type = weechat.get().hdata_get_var_type.unwrap();

        let name = LossyCString::new(name);

        unsafe {
            if hdata_get_var_type(hdata.ptr, name.as_ptr())
                != weechat_sys::WEECHAT_HDATA_INTEGER as i32
            {
                return None;
            }

            Some(hdata_integer(hdata.ptr, hdata.object, name.as_ptr()))
        }
    }
}

impl HDataType for DateTime<Utc> {
    fn hdata_value(hdata: &HData, name: &str) -> Option<Self> {
        let weechat = Weechat::from_ptr(hdata.weechat_ptr);
        let hdata_time = weechat.get().hdata_time.unwrap();
        let hdata_get_var_type = weechat.get().hdata_get_var_type.unwrap();

        let name = LossyCString::new(name);

        unsafe {
            if hdata_get_var_type(hdata.ptr, name.as_ptr())
                != weechat_sys::WEECHAT_HDATA_TIME as i32
            {
                return None;
            }

            let unix_time = hdata_time(hdata.ptr, hdata.object, name.as_ptr());
            let naive = NaiveDateTime::from_timestamp(unix_time, 0);

            Some(DateTime::from_utc(naive, Utc))
        }
    }
}

/// An opaque wrapper for a pointer stored in hdata
pub struct HDataPointer {
    ptr: *mut c_void,
    weechat: *mut t_weechat_plugin,
}

impl HDataType for HDataPointer {
    fn hdata_value(hdata: &HData, name: &str) -> Option<Self> {
        let weechat = Weechat::from_ptr(hdata.weechat_ptr);
        let hdata_pointer = weechat.get().hdata_pointer.unwrap();
        let hdata_get_var_type = weechat.get().hdata_get_var_type.unwrap();

        let name = LossyCString::new(name);

        unsafe {
            if hdata_get_var_type(hdata.ptr, name.as_ptr())
                != weechat_sys::WEECHAT_HDATA_POINTER as i32
            {
                return None;
            }

            Some(HDataPointer {
                ptr: hdata_pointer(hdata.ptr, hdata.object, name.as_ptr()),
                weechat: hdata.weechat_ptr,
            })
        }
    }
}

impl HasHData for HDataPointer {
    fn get_hdata(&self, name: &str) -> Option<HData> {
        let hdata_get =
            Weechat::from_ptr(self.weechat).get().hdata_get.unwrap();

        let name = LossyCString::new(name);

        unsafe {
            let hdata = hdata_get(self.weechat, name.as_ptr());
            if hdata.is_null() {
                None
            } else {
                Some(HData {
                    weechat_ptr: self.weechat,
                    object: self.ptr as *mut _,
                    ptr: hdata,
                })
            }
        }
    }
}
