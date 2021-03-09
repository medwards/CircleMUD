use std::ffi::CStr;
use std::io::{self, Read, Write};
use std::os::raw::{c_char, c_void};
use std::ptr::copy_nonoverlapping;

type ErrorCode = usize;

// "descriptor" will be an identifier the ffi client can use to determine which user this request
// is for, for now just handle it all globally on stdin/stdout

#[no_mangle]
pub extern "C" fn ffi_new_descriptor(descriptor_type: usize) -> *mut Descriptor {
    let ptr = Box::new(ByteStreamDescriptor::new(
        Box::new(io::stdin()),
        Box::new(io::stdout()),
    ));
    Box::into_raw(ptr)
}

#[no_mangle]
pub extern "C" fn ffi_write_to_descriptor(
    descriptor: *mut Descriptor,
    content: *const c_char,
) -> ErrorCode {
    unsafe {
        // TODO: need to review the safety requirements here file:///home/medwards/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/share/doc/rust/html/std/ffi/struct.CStr.html#method.from_ptr
        let content = CStr::from_ptr(content);
        // TODO need to ensure its not null
        descriptor
            .as_mut()
            .expect("descriptor was null")
            .write(content.to_string_lossy().to_string());
    }
    return 0;
}

#[no_mangle]
pub extern "C" fn ffi_read_from_descriptor(
    descriptor: *mut Descriptor,
    read_point: *mut c_char,
    space_left: usize,
) -> usize {
    // TODO: would rather write directly into the string if possible
    // We know this is is the max buffer size because MAX_RAW_INPUT_LENGTH is defined in structs.h
    let mut buffer = [0; 512];

    unsafe {
        let read_bytes: usize = match descriptor
            .as_mut()
            .expect("descriptor was null")
            .read(&mut buffer[0..space_left])
        {
            Ok(n) => n,
            Err(e) => {
                println!("Failed to read: {:?}:", e);
                0 // actually could return -1 here
            }
        };
        // TODO: is buffer really an i8 array?
        copy_nonoverlapping(buffer.as_ptr() as *mut i8, read_point, read_bytes);
        return read_bytes;
    }
}

#[no_mangle]
pub extern "C" fn ffi_close_descriptor(descriptor: *mut Descriptor) {
    unsafe {
        // TODO ensure its not null (or can I just drop(descriptor.as_ref().unwrap()))?
        drop(Box::from_raw(descriptor));
    }
}

// TODO: this should probably just require the Read trait
pub trait Descriptor {
    fn read(&mut self, read_point: &mut [u8]) -> Result<usize, ErrorCode>;
    fn write(&mut self, content: String) -> Result<usize, ErrorCode>;
}

pub struct ByteStreamDescriptor {
    reader: Box<dyn Read>,
    writer: Box<dyn Write>,
}

impl ByteStreamDescriptor {
    pub fn new(
        reader: Box<dyn std::io::Read + 'static>,
        writer: Box<dyn std::io::Write + 'static>,
    ) -> Self {
        Self {
            reader: reader,
            writer: writer,
        }
    }
    // no special drop impl requited
}

impl Descriptor for ByteStreamDescriptor {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, ErrorCode> {
        // TODO: don't silently drop the error
        self.reader.read(buf).map_err(|e| 0)
    }

    fn write(&mut self, content: String) -> Result<usize, ErrorCode> {
        self.writer.write(content.as_bytes()).map_err(|e| 1)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
