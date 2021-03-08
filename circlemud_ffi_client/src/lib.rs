use std::ffi::CStr;
use std::io::{self, Read};
use std::os::raw::{c_char, c_void};
use std::ptr::copy_nonoverlapping;

type ErrorCode = usize;

// "descriptor" will be an identifier the ffi client can use to determine which user this request
// is for, for now just handle it all globally on stdin/stdout

#[no_mangle]
pub extern "C" fn ffi_write_to_descriptor(
    descriptor: *const c_void,
    content: *const c_char,
) -> ErrorCode {
    unsafe {
        // TODO: need to review the safety requirements here file:///home/medwards/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/share/doc/rust/html/std/ffi/struct.CStr.html#method.from_ptr
        let content = CStr::from_ptr(content);
        println!("{}", content.to_string_lossy());
    }
    return 0;
}

#[no_mangle]
pub extern "C" fn ffi_read_from_descriptor(
    descriptor: *const c_void,
    read_point: *mut c_char,
    space_left: usize,
) -> usize {
    // TODO: would rather write directly into the string if possible
    // We know this is is the max buffer size because MAX_RAW_INPUT_LENGTH is defined in structs.h
    let mut buffer = [0; 512];

    let read_bytes: usize = match io::stdin().read(&mut buffer[0..space_left]) {
        Ok(n) => n,
        Err(e) => {
            println!("Failed to read: {:?}:", e);
            0 // actually could return -1 here
        }
    };

    unsafe {
        // TODO: is buffer really an i8 array?
        copy_nonoverlapping(buffer.as_ptr() as *mut i8, read_point, read_bytes);
    }
    return read_bytes;
}

#[no_mangle]
pub extern "C" fn ffi_close_descriptor(descriptor: *const c_void) {}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
