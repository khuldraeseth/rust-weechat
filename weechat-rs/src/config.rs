//! Weechat Configuration module

use libc::{c_char, c_int};
use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_void;
use std::ptr;

use crate::config_options::{
    BooleanOption, ColorOption, ConfigOption, IntegerOption, OptionDescription,
    OptionPointers, OptionType, StringOption,
};
use crate::{LossyCString, Weechat};
use std::borrow::Cow;
use weechat_sys::{
    t_config_file, t_config_option, t_config_section, t_weechat_plugin,
    WEECHAT_RC_OK,
};

/// Weechat configuration file
pub struct Config<T> {
    ptr: *mut t_config_file,
    weechat_ptr: *mut t_weechat_plugin,
    _config_data: Box<ConfigPointers<T>>,
    sections: HashMap<String, ConfigSection>,
}

struct ConfigPointers<T> {
    reload_cb: Option<fn(&mut T)>,
    reload_data: T,
}

/// Weechat Configuration section
pub struct ConfigSection {
    ptr: *mut t_config_section,
    config_ptr: *mut t_config_file,
    weechat_ptr: *mut t_weechat_plugin,
}

/// Represents the options when creating a new config section.
#[derive(Default)]
pub struct ConfigSectionInfo<'a, T> {
    /// Name of the config section
    pub name: &'a str,

    /// Can the user create new options?
    pub user_can_add_options: bool,
    /// Can the user delete options?
    pub user_can_delete_option: bool,

    /// A function called when an option from the section is read from the disk
    pub read_callback: Option<fn(&T)>,
    /// Data passed to the `read_callback`
    pub read_callback_data: Option<T>,

    /// A function called when the section is written to the disk
    pub write_callback: Option<fn(&T)>,
    /// Data passed to the `write_callback`
    pub write_callback_data: Option<T>,

    /// A function called when default values for the section must be written to the disk
    pub write_default_callback: Option<fn(&T)>,
    /// Data passed to the `write_default_callback`
    pub write_default_callback_data: Option<T>,

    /// A function called when a new option is created in the section
    pub create_option_callback: Option<fn(&T)>,
    /// Data passed to the `create_option_callback`
    pub create_option_callback_data: Option<T>,

    /// A function called when an option is deleted in the section
    pub delete_option_callback: Option<fn(&T)>,
    /// Data passed to the `delete_option_callback`
    pub delete_option_callback_data: Option<T>,
}

impl<T> Drop for Config<T> {
    fn drop(&mut self) {
        let weechat = Weechat::from_ptr(self.weechat_ptr);
        let config_free = weechat.get().config_free.unwrap();

        // Drop the sections first.
        self.sections.clear();

        unsafe {
            // Now drop the config.
            config_free(self.ptr)
        };
    }
}

impl Drop for ConfigSection {
    fn drop(&mut self) {
        let weechat = Weechat::from_ptr(self.weechat_ptr);

        let options_free = weechat.get().config_section_free_options.unwrap();
        let section_free = weechat.get().config_section_free.unwrap();

        unsafe {
            options_free(self.ptr);
            section_free(self.ptr);
        };
    }
}

impl<T> Config<T> {
    /// Create a new section in the configuration file.
    pub fn new_section<S: Default>(
        &mut self,
        section_info: ConfigSectionInfo<S>,
    ) -> &ConfigSection {
        let weechat = Weechat::from_ptr(self.weechat_ptr);

        let new_section = weechat.get().config_new_section.unwrap();

        let name = LossyCString::new(section_info.name);

        let ptr = unsafe {
            new_section(
                self.ptr,
                name.as_ptr(),
                section_info.user_can_add_options as i32,
                section_info.user_can_delete_option as i32,
                None,
                ptr::null_mut(),
                ptr::null_mut(),
                None,
                ptr::null_mut(),
                ptr::null_mut(),
                None,
                ptr::null_mut(),
                ptr::null_mut(),
                None,
                ptr::null_mut(),
                ptr::null_mut(),
                None,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        let section = ConfigSection {
            ptr,
            config_ptr: self.ptr,
            weechat_ptr: weechat.ptr,
        };
        self.sections.insert(section_info.name.to_string(), section);
        &self.sections[section_info.name]
    }

    /// Load configuration data from the disk
    pub fn read(&self) {
        let weechat = Weechat::from_ptr(self.weechat_ptr);

        let config_read = weechat.get().config_read.unwrap();

        unsafe {
            config_read(self.ptr);
        }
    }

    /// Save this config file to the disk
    pub fn write(&self) {
        let weechat = Weechat::from_ptr(self.weechat_ptr);

        let config_write = weechat.get().config_write.unwrap();

        unsafe {
            config_write(self.ptr);
        }
    }
}

type WeechatOptChangeCbT = unsafe extern "C" fn(
    pointer: *const c_void,
    _data: *mut c_void,
    option_pointer: *mut t_config_option,
);

type WeechatOptCheckCbT = unsafe extern "C" fn(
    pointer: *const c_void,
    _data: *mut c_void,
    option_pointer: *mut t_config_option,
    value: *const c_char,
) -> c_int;

impl ConfigSection {
    /// Create a new string Weechat configuration option.
    pub fn new_string_option<D>(
        &self,
        name: &str,
        description: &str,
        default_value: &str,
        value: &str,
        null_allowed: bool,
        change_cb: Option<fn(&mut D, &StringOption)>,
        change_cb_data: Option<D>,
    ) -> StringOption
    where
        D: Default,
    {
        let ptr = self.new_option(
            OptionDescription {
                name,
                description,
                option_type: OptionType::String,
                default_value,
                value,
                null_allowed,
                ..Default::default()
            },
            None,
            None::<String>,
            change_cb,
            change_cb_data,
            None,
            None::<String>,
        );
        StringOption {
            ptr,
            weechat_ptr: self.weechat_ptr,
        }
    }

    /// Create a new boolean Weechat configuration option.
    pub fn new_boolean_option<D>(
        &self,
        name: &str,
        description: &str,
        default_value: bool,
        value: bool,
        null_allowed: bool,
        change_cb: Option<fn(&mut D, &BooleanOption)>,
        change_cb_data: Option<D>,
    ) -> BooleanOption
    where
        D: Default,
    {
        let value = if value { "on" } else { "off" };
        let default_value = if default_value { "on" } else { "off" };
        let ptr = self.new_option(
            OptionDescription {
                name,
                description,
                option_type: OptionType::Boolean,
                default_value,
                value,
                null_allowed,
                ..Default::default()
            },
            None,
            None::<String>,
            change_cb,
            change_cb_data,
            None,
            None::<String>,
        );
        BooleanOption {
            ptr,
            weechat_ptr: self.weechat_ptr,
        }
    }

    /// Create a new integer Weechat configuration option.
    pub fn new_integer_option<D>(
        &self,
        name: &str,
        description: &str,
        string_values: &str,
        min: i32,
        max: i32,
        default_value: &str,
        value: &str,
        null_allowed: bool,
        change_cb: Option<fn(&mut D, &IntegerOption)>,
        change_cb_data: Option<D>,
    ) -> IntegerOption
    where
        D: Default,
    {
        let ptr = self.new_option(
            OptionDescription {
                name,
                option_type: OptionType::Integer,
                description,
                string_values,
                min,
                max,
                default_value,
                value,
                null_allowed,
            },
            None,
            None::<String>,
            change_cb,
            change_cb_data,
            None,
            None::<String>,
        );
        IntegerOption {
            ptr,
            weechat_ptr: self.weechat_ptr,
        }
    }

    /// Create a new color Weechat configuration option.
    pub fn new_color_option<D>(
        &self,
        name: &str,
        description: &str,
        default_value: &str,
        value: &str,
        null_allowed: bool,
        change_cb: Option<fn(&mut D, &ColorOption)>,
        change_cb_data: Option<D>,
    ) -> ColorOption
    where
        D: Default,
    {
        let ptr = self.new_option(
            OptionDescription {
                name,
                description,
                option_type: OptionType::Color,
                default_value,
                value,
                null_allowed,
                ..Default::default()
            },
            None,
            None::<String>,
            change_cb,
            change_cb_data,
            None,
            None::<String>,
        );
        ColorOption {
            ptr,
            weechat_ptr: self.weechat_ptr,
        }
    }

    fn new_option<'a, T, A, B, C>(
        &self,
        option_description: OptionDescription,
        check_cb: Option<fn(&mut A, &T, Cow<str>)>,
        check_cb_data: Option<A>,
        change_cb: Option<fn(&mut B, &T)>,
        change_cb_data: Option<B>,
        delete_cb: Option<fn(&mut C, &T)>,
        delete_cb_data: Option<C>,
    ) -> *mut t_config_option
    where
        T: ConfigOption<'static>,
        A: Default,
        B: Default,
        C: Default,
    {
        unsafe extern "C" fn c_check_cb<T, A, B, C>(
            pointer: *const c_void,
            _data: *mut c_void,
            option_pointer: *mut t_config_option,
            value: *const c_char,
        ) -> c_int
        where
            T: ConfigOption<'static>,
        {
            let value = CStr::from_ptr(value).to_string_lossy();
            let pointers: &mut OptionPointers<T, A, B, C> =
                { &mut *(pointer as *mut OptionPointers<T, A, B, C>) };

            let option = T::from_ptrs(option_pointer, pointers.weechat_ptr);

            let data = &mut pointers.check_cb_data;

            if let Some(callback) = pointers.check_cb {
                callback(data, &option, value)
            };

            WEECHAT_RC_OK
        }

        unsafe extern "C" fn c_change_cb<T, A, B, C>(
            pointer: *const c_void,
            _data: *mut c_void,
            option_pointer: *mut t_config_option,
        ) where
            T: ConfigOption<'static>,
        {
            let pointers: &mut OptionPointers<T, A, B, C> =
                { &mut *(pointer as *mut OptionPointers<T, A, B, C>) };

            let option = T::from_ptrs(option_pointer, pointers.weechat_ptr);

            let data = &mut pointers.change_cb_data;

            if let Some(callback) = pointers.change_cb {
                callback(data, &option)
            };
        }

        unsafe extern "C" fn c_delete_cb<T, A, B, C>(
            pointer: *const c_void,
            _data: *mut c_void,
            option_pointer: *mut t_config_option,
        ) where
            T: ConfigOption<'static>,
        {
            let pointers: &mut OptionPointers<T, A, B, C> =
                { &mut *(pointer as *mut OptionPointers<T, A, B, C>) };

            let option = T::from_ptrs(option_pointer, pointers.weechat_ptr);

            let data = &mut pointers.delete_cb_data;

            if let Some(callback) = pointers.delete_cb {
                callback(data, &option)
            };
        }

        let weechat = Weechat::from_ptr(self.weechat_ptr);

        let name = LossyCString::new(option_description.name);
        let description = LossyCString::new(option_description.description);
        let option_type =
            LossyCString::new(option_description.option_type.as_str());
        let string_values = LossyCString::new(option_description.string_values);
        let default_value = LossyCString::new(option_description.default_value);
        let value = LossyCString::new(option_description.value);

        let option_pointers = Box::new(OptionPointers::<T, A, B, C> {
            weechat_ptr: self.weechat_ptr,
            check_cb: check_cb,
            check_cb_data: check_cb_data.unwrap_or_default(),
            change_cb: change_cb,
            change_cb_data: change_cb_data.unwrap_or_default(),
            delete_cb: delete_cb,
            delete_cb_data: delete_cb_data.unwrap_or_default(),
        });

        // TODO this leaks curently.
        let option_pointers_ref: &OptionPointers<T, A, B, C> =
            Box::leak(option_pointers);

        let c_check_cb: Option<WeechatOptCheckCbT> = match check_cb {
            Some(_) => Some(c_check_cb::<T, A, B, C>),
            None => None,
        };

        let c_change_cb: Option<WeechatOptChangeCbT> = match change_cb {
            Some(_) => Some(c_change_cb::<T, A, B, C>),
            None => None,
        };

        let c_delete_cb: Option<WeechatOptChangeCbT> = match delete_cb {
            Some(_) => Some(c_delete_cb::<T, A, B, C>),
            None => None,
        };

        let config_new_option = weechat.get().config_new_option.unwrap();
        unsafe {
            config_new_option(
                self.config_ptr,
                self.ptr,
                name.as_ptr(),
                option_type.as_ptr(),
                description.as_ptr(),
                string_values.as_ptr(),
                option_description.min,
                option_description.max,
                default_value.as_ptr(),
                value.as_ptr(),
                option_description.null_allowed as i32,
                c_check_cb,
                option_pointers_ref as *const _ as *const c_void,
                ptr::null_mut(),
                c_change_cb,
                option_pointers_ref as *const _ as *const c_void,
                ptr::null_mut(),
                c_delete_cb,
                option_pointers_ref as *const _ as *const c_void,
                ptr::null_mut(),
            )
        }
    }
}

type WeechatReloadT = unsafe extern "C" fn(
    pointer: *const c_void,
    _data: *mut c_void,
    _config_pointer: *mut t_config_file,
) -> c_int;

/// Configuration file part of the weechat API.
impl Weechat {
    /// Create a new Weechat configuration file, returns a `Config` object.
    /// The configuration file is freed when the `Config` object is dropped.
    /// * `name` - Name of the new configuration file
    /// * `reload_callback` - Callback that will be called when the
    /// configuration file is reloaded.
    /// * `reload_data` - Data that will be taken over by weechat and passed
    /// to the reload callback, this data will be freed when the `Config`
    /// object returned by this method is dropped.
    pub fn config_new<T: Default>(
        &self,
        name: &str,
        reload_callback: Option<fn(&mut T)>,
        reload_data: Option<T>,
    ) -> Config<T> {
        unsafe extern "C" fn c_reload_cb<T>(
            pointer: *const c_void,
            _data: *mut c_void,
            _config_pointer: *mut t_config_file,
        ) -> c_int {
            let pointers: &mut ConfigPointers<T> =
                { &mut *(pointer as *mut ConfigPointers<T>) };

            let data = &mut pointers.reload_data;

            if let Some(callback) = pointers.reload_cb {
                callback(data)
            }

            WEECHAT_RC_OK
        }

        let c_name = LossyCString::new(name);

        let config_pointers = Box::new(ConfigPointers::<T> {
            reload_cb: reload_callback,
            reload_data: reload_data.unwrap_or_default(),
        });
        let config_pointers_ref = Box::leak(config_pointers);

        let c_reload_cb: Option<WeechatReloadT> = match reload_callback {
            Some(_) => Some(c_reload_cb::<T>),
            None => None,
        };

        let config_new = self.get().config_new.unwrap();
        let config_ptr = unsafe {
            config_new(
                self.ptr,
                c_name.as_ptr(),
                c_reload_cb,
                config_pointers_ref as *const _ as *const c_void,
                ptr::null_mut(),
            )
        };

        let config_data = unsafe { Box::from_raw(config_pointers_ref) };
        Config {
            ptr: config_ptr,
            weechat_ptr: self.ptr,
            _config_data: config_data,
            sections: HashMap::new(),
        }
    }
}
