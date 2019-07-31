#![warn(missing_docs)]

//! Weechat Hook module.
//! Weechat hooks are used for many different things, to create commands, to
//! listen to events on a file descriptor, add completions to weechat, etc.
//! This module contains hook creation methods for the `Weechat` object.

use libc::{c_char, c_int};
use std::os::raw::c_void;
use std::os::unix::io::AsRawFd;
use std::ptr;

use weechat_sys::{t_gui_buffer, t_hook, t_weechat_plugin, WEECHAT_RC_OK};

use crate::{ArgsWeechat, Buffer, LossyCString, Weechat};

/// Weechat Hook type. The hook is unhooked automatically when the object is
/// dropped.
pub(crate) struct Hook {
    pub(crate) ptr: *mut t_hook,
    pub(crate) weechat_ptr: *mut t_weechat_plugin,
}

/// Hook for a weechat command, the command is removed when the object is
/// dropped.
pub struct CommandHook<T> {
    _hook: Hook,
    _hook_data: Box<CommandHookData<T>>,
}

/// Setting for the FdHook.
pub enum FdHookMode {
    /// Catch read events.
    Read,
    /// Catch write events.
    Write,
    /// Catch read and write events.
    ReadWrite,
}

/// Hook for a file descriptor, the hook is removed when the object is dropped.
pub struct FdHook<T, F> {
    _hook: Hook,
    _hook_data: Box<FdHookData<T, F>>,
}

struct FdHookData<T, F> {
    callback: fn(&T, fd_object: &mut F),
    callback_data: T,
    fd_object: F,
}

struct CommandHookData<T> {
    callback: fn(&T, Buffer, ArgsWeechat),
    callback_data: T,
    weechat_ptr: *mut t_weechat_plugin,
}

impl FdHookMode {
    pub(crate) fn as_tuple(&self) -> (i32, i32) {
        let read = match self {
            FdHookMode::Read => 1,
            FdHookMode::ReadWrite => 1,
            FdHookMode::Write => 0,
        };

        let write = match self {
            FdHookMode::Read => 0,
            FdHookMode::ReadWrite => 1,
            FdHookMode::Write => 1,
        };
        (read, write)
    }
}

impl Drop for Hook {
    fn drop(&mut self) {
        let weechat = Weechat::from_ptr(self.weechat_ptr);
        let unhook = weechat.get().unhook.unwrap();
        unsafe { unhook(self.ptr) };
    }
}

#[derive(Default)]
/// Description for a weechat command that should will be hooked.
/// The fields of this struct accept the same string formats that are described
/// in the weechat API documentation.
pub struct CommandDescription<'a> {
    /// Name of the command.
    pub name: &'a str,
    /// Description for the command (displayed with `/help command`)
    pub description: &'a str,
    /// Arguments for the command (displayed with `/help command`)
    pub args: &'a str,
    /// Description for the command arguments (displayed with `/help command`)
    pub args_description: &'a str,
    /// Completion template for the command.
    pub completion: &'a str,
}

impl Weechat {
    /// Create a new weechat command. Returns the hook of the command. The
    /// command is unhooked if the hook is dropped.
    pub fn hook_command<T>(
        &self,
        command_info: CommandDescription,
        callback: fn(data: &T, buffer: Buffer, args: ArgsWeechat),
        callback_data: Option<T>,
    ) -> CommandHook<T>
    where
        T: Default,
    {
        unsafe extern "C" fn c_hook_cb<T>(
            pointer: *const c_void,
            _data: *mut c_void,
            buffer: *mut t_gui_buffer,
            argc: i32,
            argv: *mut *mut c_char,
            _argv_eol: *mut *mut c_char,
        ) -> c_int {
            let hook_data: &mut CommandHookData<T> =
                { &mut *(pointer as *mut CommandHookData<T>) };
            let buffer = Buffer::from_ptr(hook_data.weechat_ptr, buffer);
            let callback = hook_data.callback;
            let callback_data = &hook_data.callback_data;
            let args = ArgsWeechat::new(argc, argv);

            callback(callback_data, buffer, args);

            WEECHAT_RC_OK
        }

        let name = LossyCString::new(command_info.name);
        let description = LossyCString::new(command_info.description);
        let args = LossyCString::new(command_info.args);
        let args_description = LossyCString::new(command_info.args_description);
        let completion = LossyCString::new(command_info.completion);

        let data = Box::new(CommandHookData {
            callback,
            callback_data: callback_data.unwrap_or_default(),
            weechat_ptr: self.ptr,
        });

        let data_ref = Box::leak(data);

        let hook_command = self.get().hook_command.unwrap();
        let hook_ptr = unsafe {
            hook_command(
                self.ptr,
                name.as_ptr(),
                description.as_ptr(),
                args.as_ptr(),
                args_description.as_ptr(),
                completion.as_ptr(),
                Some(c_hook_cb::<T>),
                data_ref as *const _ as *const c_void,
                ptr::null_mut(),
            )
        };
        let hook_data = unsafe { Box::from_raw(data_ref) };
        let hook = Hook {
            ptr: hook_ptr,
            weechat_ptr: self.ptr,
        };

        CommandHook::<T> {
            _hook: hook,
            _hook_data: hook_data,
        }
    }

    /// Hook an object that can be turned into a raw file descriptor.
    /// Returns the hook object.
    /// * `fd_object` - An object for wich the file descriptor will be watched
    ///     and the callback called when read or write operations can happen
    ///     on it.
    /// * `mode` - Configure the hook to watch for writes, reads or both on the
    ///     file descriptor.
    /// * `callback` - A function that will be called if a watched event on the
    ///     file descriptor happends.
    /// * `callback_data` - Data that will be passed to the callback every time
    ///     the callback runs. This data will be freed when the hook is
    ///     unhooked.
    pub fn hook_fd<T, F>(
        &self,
        fd_object: F,
        mode: FdHookMode,
        callback: fn(data: &T, fd_object: &mut F),
        callback_data: Option<T>,
    ) -> FdHook<T, F>
    where
        T: Default,
        F: AsRawFd,
    {
        unsafe extern "C" fn c_hook_cb<T, F>(
            pointer: *const c_void,
            _data: *mut c_void,
            _fd: i32,
        ) -> c_int {
            let hook_data: &mut FdHookData<T, F> =
                { &mut *(pointer as *mut FdHookData<T, F>) };
            let callback = hook_data.callback;
            let callback_data = &hook_data.callback_data;
            let fd_object = &mut hook_data.fd_object;

            callback(callback_data, fd_object);

            WEECHAT_RC_OK
        }

        let fd = fd_object.as_raw_fd();

        let data = Box::new(FdHookData {
            callback,
            callback_data: callback_data.unwrap_or_default(),
            fd_object,
        });

        let data_ref = Box::leak(data);
        let hook_fd = self.get().hook_fd.unwrap();
        let (read, write) = mode.as_tuple();

        let hook_ptr = unsafe {
            hook_fd(
                self.ptr,
                fd,
                read,
                write,
                0,
                Some(c_hook_cb::<T, F>),
                data_ref as *const _ as *const c_void,
                ptr::null_mut(),
            )
        };
        let hook_data = unsafe { Box::from_raw(data_ref) };
        let hook = Hook {
            ptr: hook_ptr,
            weechat_ptr: self.ptr,
        };

        FdHook::<T, F> {
            _hook: hook,
            _hook_data: hook_data,
        }
    }
}
