use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr::copy_nonoverlapping;

mod descriptor;
use descriptor::{/*ByteStreamDescriptor,*/ DescriptorId, DescriptorManager, ErrorCode};
mod slack_descriptor;

// "descriptor" will be an identifier the ffi client can use to determine which user this request
// is for, for now just handle it all globally on stdin/stdout

#[no_mangle]
pub extern "C" fn ffi_create_descriptor_manager() -> *mut slack_descriptor::SlackDescriptorManager {
    Box::into_raw(Box::new(slack_descriptor::SlackDescriptorManager::new(
        &std::env::var("SLACK_SIGNING_SECRET")
            .expect("SLACK_SIGNING_SECRET not in the environment"),
        std::env::var("SLACK_BOT_TOKEN")
            .expect("SLACK_BOT_TOKEN not in the environment")
            .into(),
    )))
}

#[no_mangle]
pub extern "C" fn ffi_new_descriptor(
    manager: *mut slack_descriptor::SlackDescriptorManager,
    descriptor_type: usize,
) -> *const DescriptorId {
    unsafe {
        match manager
            .as_mut()
            .expect("manager was null")
            .get_new_descriptor()
        {
            Some(descriptor) => {
                println!("Found new descriptor: {:?}", descriptor);
                Box::into_raw(Box::new(descriptor.clone()))
            }
            None => std::ptr::null(),
        }
    }
    /*
    let ptr = Box::new(ByteStreamDescriptor::new(
        Box::new(io::stdin()),
        Box::new(io::stdout()),
    ));
    Box::into_raw(ptr)
    */
}

#[no_mangle]
pub extern "C" fn ffi_close_descriptor(
    manager: *mut slack_descriptor::SlackDescriptorManager,
    identifier: *mut DescriptorId,
) {
    unsafe {
        // TODO ensure its not null (or can I just drop(descriptor.as_ref().unwrap()))?
        let manager = manager.as_mut().expect("manager was null");
        let identifier = Box::from_raw(identifier);
        manager.close_descriptor(&identifier);
        drop(identifier);
    }
}

#[no_mangle]
pub extern "C" fn ffi_write_to_descriptor(
    manager: *mut slack_descriptor::SlackDescriptorManager,
    identifier: *const DescriptorId,
    content: *const c_char,
) -> isize {
    unsafe {
        // TODO: need to review the safety requirements here file:///home/medwards/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/share/doc/rust/html/std/ffi/struct.CStr.html#method.from_ptr
        let content = CStr::from_ptr(content);
        // TODO need to ensure its not null
        let manager = manager.as_mut().expect("manager was null");
        let identifier = identifier.as_ref().expect("descriptor identifier was null");
        manager
            .get_descriptor(&identifier)
            .expect("descriptor was not found")
            .write(content.to_bytes())
            .map(|size| size as isize) // TODO: tryinto instead
            .unwrap_or(-1)
    }
}

#[no_mangle]
pub extern "C" fn ffi_read_from_descriptor(
    manager: *mut slack_descriptor::SlackDescriptorManager,
    identifier: *const DescriptorId,
    read_point: *mut c_char,
    space_left: usize,
    out_read_bytes: *mut usize,
) -> isize {
    // TODO: would rather write directly into the string if possible
    // We know this is is the max buffer size because MAX_RAW_INPUT_LENGTH is defined in structs.h
    let mut buffer = [0; 512];

    unsafe {
        let manager = manager.as_mut().expect("descriptor was null");
        let identifier = identifier.as_ref().expect("descriptor identifier was null");
        let read_bytes: usize = match manager
            .get_descriptor(&identifier)
            .expect("Failed to lock descriptor")
            .read(&mut buffer[0..space_left])
        {
            Ok(n) => n,
            Err(e) => {
                println!("Failed to read: {:?}:", e);
                return -1;
            }
        };
        // TODO: is buffer really an i8 array?
        copy_nonoverlapping(buffer.as_ptr() as *mut i8, read_point, read_bytes);
        *out_read_bytes = read_bytes;
        return 0;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
