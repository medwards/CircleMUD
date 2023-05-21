use std::ffi::CStr;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::os::raw::c_char;
use std::os::raw::c_uchar;

#[no_mangle]
pub extern "C" fn new_descriptor_manager(port: u16) -> *mut DescriptorManager {
    match DescriptorManager::new(port) {
        Ok(manager) => Box::into_raw(Box::new(manager)),
        // TODO log, then return an error code?
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn close_descriptor_manager(mut manager: *mut DescriptorManager) -> i32 {
    if manager.is_null() {
        return -1;
    }
    // SAFETY: `from_raw` can result in a double-free - the pointer is set to null and after
    // dropping and pointer is checked if it is null before dropping
    unsafe {
        drop(Box::from_raw(manager));
        manager = std::ptr::null_mut();
        return 0;
    }
}

#[no_mangle]
pub extern "C" fn new_descriptor(manager: *mut DescriptorManager) -> *mut Descriptor {
    if manager.is_null() {
        return std::ptr::null_mut();
    }

    unsafe {
        match (*manager).listener.accept() {
            Ok((stream, _socket_addr)) => {
                if stream.set_nonblocking(true).is_err() {
                    return std::ptr::null_mut();
                }
                Box::into_raw(Box::new(Descriptor::new(stream)))
            }
            Err(_) => std::ptr::null_mut(),
        }
    }
}

#[no_mangle]
pub extern "C" fn close_descriptor(
    mut _manager: *mut DescriptorManager,
    mut descriptor: *mut Descriptor,
) -> i32 {
    if descriptor.is_null() {
        return -1;
    }
    unsafe {
        drop(Box::from_raw(descriptor));
        descriptor = std::ptr::null_mut();
        return 1;
    }
}

#[no_mangle]
pub extern "C" fn read_from_descriptor(
    mut manager: *mut DescriptorManager,
    mut descriptor: *mut Descriptor,
    read_point: *mut c_uchar,
    space_left: usize,
) -> isize {
    if descriptor.is_null() || read_point.is_null() || space_left == 0 {
        return -1;
    }
    unsafe {
        let buffer = std::slice::from_raw_parts_mut(read_point, space_left);
        match (*descriptor).stream.read(buffer) {
            Ok(bytes) => isize::try_from(bytes).unwrap_or(-1),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => 0,
            Err(e) => {
                dbg!(e);
                -1
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn write_to_descriptor(
    _manager: *mut DescriptorManager,
    descriptor: *mut Descriptor,
    content: *const c_char,
) -> isize {
    if descriptor.is_null() {
        return -1;
    }
    unsafe {
        match (*descriptor)
            .stream
            .write(CStr::from_ptr(content).to_bytes())
        {
            Ok(written) => isize::try_from(written).unwrap_or(-1),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => 0,
            Err(e) => {
                dbg!(e);
                -1
            }
        }
    }
}

pub struct DescriptorManager {
    listener: TcpListener,
}

impl DescriptorManager {
    fn new(listen_port: u16) -> Result<DescriptorManager, std::io::Error> {
        let listener = TcpListener::bind(("0.0.0.0", listen_port))?;
        listener.set_nonblocking(true)?;
        Ok(DescriptorManager { listener })
    }
}

pub struct Descriptor {
    stream: TcpStream,
}

impl Descriptor {
    fn new(stream: TcpStream) -> Descriptor {
        Descriptor { stream }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
