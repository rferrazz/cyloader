extern crate libc;
extern crate cyapi;
extern crate log;

use libc::{c_char, c_int};
use std::ffi::CStr;
use std::cell::RefCell;
use std::io::Error;
use log::{warn};
use std::{slice, ptr};


thread_local!{
    static LAST_ERROR: RefCell<Option<Box<Error>>> = RefCell::new(None);
}

#[no_mangle]
/// update_psoc reads a .cyacd update_file and updates the PSoC via the specified serial_port.
/// Returns 0 on success, 1 on an update session initialization error, 2 on an upload error.
/// 
/// To get a detailed error of what went wrong you can use last_error_message()
pub extern "C" fn update_psoc(update_file: *const c_char, serial_port: *const c_char) -> c_int {
    let s: &CStr = unsafe { CStr::from_ptr(serial_port) };
    let f: &CStr = unsafe { CStr::from_ptr(update_file) };
    
    let mut session = match cyapi::UpdateSession::new(String::from(s.to_str().unwrap())) {
        Ok(session) => session,
        Err(error) => {
            LAST_ERROR.with(|prev| {
                *prev.borrow_mut() = Some(Box::new(error));
            });

            return 1;
        },
    };

    if let Err(error) = session.update(String::from(f.to_str().unwrap())) {
        LAST_ERROR.with(|prev| {
            *prev.borrow_mut() = Some(Box::new(error));
        });

        return 2;
    }
    
    return 0;
}

#[no_mangle]
/// last_error_length returns the size of the latest error message.
/// It is useful to preallocate the buffer to read the error message
pub extern "C" fn last_error_length() -> c_int {
    LAST_ERROR.with(|prev| match *prev.borrow() {
        Some(ref err) => err.to_string().len() as c_int + 1,
        None => 0,
    })
}

#[no_mangle]
/// last_error_message fills buffer with the latest error message.
/// It returns the buffer length on success, -1 otherwise
pub extern "C" fn last_error_message(buffer: *mut c_char, length: c_int) -> c_int {
    if buffer.is_null() {
        warn!("Null pointer passed into last_error_message() as the buffer");
        return -1;
    }

    let last_error = match LAST_ERROR.with(|prev| prev.borrow_mut().take()) {
        Some(err) => err,
        None => return 0,
    };

    let error_message = last_error.to_string();

    unsafe {
        let buffer = slice::from_raw_parts_mut(buffer as *mut u8, length as usize);

        if error_message.len() >= buffer.len()
        {
            warn!("Buffer provided for writing the last error message is too small.");
            warn!(
                "Expected at least {} bytes but got {}",
                error_message.len() + 1,
                buffer.len()
            );
            return -1;
        }

        ptr::copy_nonoverlapping(
            error_message.as_ptr(),
            buffer.as_mut_ptr(),
            error_message.len(),
        );

        // Add a trailing null so people using the string as a `char *` don't
        // accidentally read into garbage.
        buffer[error_message.len()] = 0;
    }

    error_message.len() as c_int
}