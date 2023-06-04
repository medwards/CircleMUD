mod socket_libc;

use std::cmp::min;
use std::ffi::CStr;
use std::ffi::CString;
use std::io::Read;
use std::io::Write;
use std::os::raw::c_char;
use std::os::raw::c_uchar;

#[no_mangle]
pub extern "C" fn new_descriptor_manager(port: u16) -> *mut socket_libc::DescriptorManager {
    match socket_libc::DescriptorManager::new(port) {
        Ok(manager) => Box::into_raw(Box::new(manager)),
        // TODO log, then return an error code?
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn close_descriptor_manager(manager: *mut socket_libc::DescriptorManager) -> i32 {
    if manager.is_null() {
        return -1;
    }
    // SAFETY: `from_raw` can result in a double-free - the pointer is set to null and after
    // dropping and pointer is checked if it is null before dropping
    unsafe {
        Box::from_raw(manager);
        return 0;
    }
}

#[no_mangle]
pub extern "C" fn block_until_descriptor(mut manager: *mut socket_libc::DescriptorManager) -> i32 {
    if manager.is_null() {
        return -1;
    }
    // SAFETY: `from_raw` can result in a double-free - the pointer is set to null and after
    // dropping and pointer is checked if it is null before dropping
    unsafe {
        match (*manager).block_until_descriptor() {
            Ok(_) => 0,
            Err(_) => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn new_descriptor(
    manager: *mut socket_libc::DescriptorManager,
) -> *mut socket_libc::Descriptor {
    if manager.is_null() {
        return std::ptr::null_mut();
    }

    unsafe {
        match (*manager).new_descriptor() {
            Ok(descriptor) => Box::into_raw(descriptor),
            Err(_) => std::ptr::null_mut(),
        }
    }
}

#[no_mangle]
pub extern "C" fn close_descriptor(
    mut _manager: *mut socket_libc::DescriptorManager,
    descriptor: *mut socket_libc::Descriptor,
) -> i32 {
    if descriptor.is_null() {
        return -1;
    }
    unsafe {
        Box::from_raw(descriptor);
        return 0;
    }
}

#[no_mangle]
pub extern "C" fn get_descriptor_hostname(
    mut manager: *mut socket_libc::DescriptorManager,
    mut descriptor: *mut socket_libc::Descriptor,
    read_point: *mut c_uchar,
    space_left: usize,
) -> isize {
    if manager.is_null() || descriptor.is_null() || read_point.is_null() || space_left == 0 {
        return -1;
    }

    unsafe {
        let hostname_bytes = (*descriptor).hostname.as_str().as_bytes();
        if let Ok(c_hostname) =
            CString::new(&hostname_bytes[..min(hostname_bytes.len(), space_left - 1)])
        {
            let buffer = std::slice::from_raw_parts_mut(
                read_point,
                min(space_left, c_hostname.as_bytes_with_nul().len()),
            );
            buffer.copy_from_slice(c_hostname.as_bytes_with_nul());
        } else {
            return -1;
        }

        return 0;
    }
}

#[no_mangle]
pub extern "C" fn read_from_descriptor(
    mut manager: *mut socket_libc::DescriptorManager,
    mut descriptor: *mut socket_libc::Descriptor,
    read_point: *mut c_uchar,
    space_left: usize,
) -> isize {
    if manager.is_null() || descriptor.is_null() || read_point.is_null() || space_left == 0 {
        return -1;
    }

    unsafe {
        let buffer = std::slice::from_raw_parts_mut(read_point, space_left);
        match (*descriptor).read(buffer) {
            Ok(bytes) => isize::try_from(bytes).unwrap_or(-1),
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::Interrupted =>
            {
                0
            }
            Err(e) => {
                dbg!(e);
                -1
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn write_to_descriptor(
    manager: *mut socket_libc::DescriptorManager,
    descriptor: *mut socket_libc::Descriptor,
    content: *const c_char,
) -> isize {
    if manager.is_null() || descriptor.is_null() || content.is_null() {
        return -1;
    }
    unsafe {
        match (*descriptor).write(CStr::from_ptr(content).to_bytes()) {
            Ok(written) => isize::try_from(written).unwrap_or(-1),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => 0,
            Err(e) => {
                dbg!(e);
                -1
            }
        }
    }
}
