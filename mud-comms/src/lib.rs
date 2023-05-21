use core::num;
use std::cmp::min;
use std::ffi::CStr;
use std::ffi::CString;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::mem;
use std::mem::size_of;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::os::raw::c_char;
use std::os::raw::c_uchar;

use libc::accept;
use libc::bind;
use libc::c_int;
use libc::c_void;
use libc::close;
use libc::fcntl;
use libc::fd_set;
use libc::in_addr;
use libc::listen;
use libc::read;
use libc::select;
use libc::send;
use libc::sockaddr;
use libc::sockaddr_in;
use libc::socket;
use libc::timeval;
use libc::AF_INET;
use libc::FD_ISSET;
use libc::FD_SET;
use libc::F_GETFL;
use libc::F_SETFL;
use libc::INADDR_ANY;
use libc::O_NONBLOCK;
use libc::PF_INET;
use libc::SOCK_STREAM;

#[no_mangle]
pub extern "C" fn new_descriptor_manager(port: u16) -> *mut DescriptorManager {
    match DescriptorManager::new(port) {
        Ok(manager) => Box::into_raw(Box::new(manager)),
        // TODO log, then return an error code?
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn close_descriptor_manager(manager: *mut DescriptorManager) -> i32 {
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
pub extern "C" fn block_until_descriptor(mut manager: *mut DescriptorManager) -> i32 {
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
pub extern "C" fn new_descriptor(manager: *mut DescriptorManager) -> *mut Descriptor {
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
    mut _manager: *mut DescriptorManager,
    descriptor: *mut Descriptor,
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
    mut manager: *mut DescriptorManager,
    mut descriptor: *mut Descriptor,
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
    mut manager: *mut DescriptorManager,
    mut descriptor: *mut Descriptor,
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
    manager: *mut DescriptorManager,
    descriptor: *mut Descriptor,
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

pub struct DescriptorManager {
    socket: c_int,
}

// byte ordering helpers from sys::common::net
pub fn htons(u: u16) -> u16 {
    u.to_be()
}

pub fn htonl(u: u32) -> u32 {
    u.to_be()
}

pub fn ntohs(u: u16) -> u16 {
    u16::from_be(u)
}

pub fn ntohl(u: u32) -> u32 {
    u32::from_be(u)
}

impl DescriptorManager {
    fn new(listen_port: u16) -> Result<DescriptorManager, std::io::Error> {
        unsafe {
            let s = socket(PF_INET, SOCK_STREAM, 0);
            if s < 0 {
                return Err(std::io::Error::new(ErrorKind::Other, "libc::socket failed"));
            }
            // set_sendbuf
            let addr_in = sockaddr_in {
                sin_family: AF_INET as u16,
                sin_port: htons(listen_port),
                sin_addr: in_addr {
                    s_addr: htonl(INADDR_ANY),
                }, // TODO: specify the address?
                sin_zero: [0; 8],
            };
            let x = (&addr_in) as *const sockaddr_in;
            let y = x as *const sockaddr;
            if bind(s, y, size_of::<sockaddr_in>() as u32) < 0 {
                return Err(std::io::Error::new(ErrorKind::Other, "libc::bind failed"));
            }
            // set nonblocking
            let mut flags = fcntl(s, F_GETFL, 0);
            if flags < 0 {
                return Err(std::io::Error::new(ErrorKind::Other, "libc::fcntl failed"));
            }
            flags |= O_NONBLOCK;
            if fcntl(s, F_SETFL, flags) < 0 {
                return Err(std::io::Error::new(ErrorKind::Other, "libc::fcntl failed"));
            }

            // listen on the socket
            if listen(s, 5) < 0 {
                return Err(std::io::Error::new(ErrorKind::Other, "libc::listen failed"));
            }
            Ok(DescriptorManager { socket: s })
        }
    }

    fn block_until_descriptor(&self) -> Result<(), std::io::Error> {
        unsafe {
            let mut input_set: fd_set = mem::zeroed();
            let mut output_set: fd_set = mem::zeroed();
            let mut exc_set: fd_set = mem::zeroed();
            FD_SET(self.socket, &mut input_set as *mut fd_set);
            if select(
                self.socket + 1,
                &mut input_set as *mut fd_set,
                &mut output_set as *mut fd_set,
                &mut exc_set as *mut fd_set,
                std::ptr::null_mut(),
            ) < 0
            {
                return dbg!(Err(std::io::Error::last_os_error()));
            }
        }
        Ok(())
    }

    fn new_descriptor(&self) -> Result<Box<Descriptor>, std::io::Error> {
        unsafe {
            // Maybe use FD_ZERO?
            let mut input_set: fd_set = mem::zeroed();
            let mut output_set: fd_set = mem::zeroed();
            let mut exc_set: fd_set = mem::zeroed();
            let mut timeout: timeval = mem::zeroed();
            FD_SET(self.socket, &mut input_set as *mut fd_set);
            if select(
                self.socket + 1,
                &mut input_set as *mut fd_set,
                &mut output_set as *mut fd_set,
                &mut exc_set as *mut fd_set,
                &mut timeout as *mut timeval,
            ) < 0
            {
                return Err(std::io::Error::new(ErrorKind::Other, "libc::select failed"));
            }

            // there is a descriptor
            let mut peer: sockaddr_in = mem::zeroed();
            let mut peer_len = size_of::<sockaddr_in>();
            let file_descriptor = accept(
                self.socket,
                &mut peer as *mut sockaddr_in as *mut sockaddr,
                &mut peer_len as *mut usize as *mut u32,
            );
            if file_descriptor < 0 {
                return Err(std::io::Error::new(ErrorKind::Other, "libc::accept failed"));
            }

            let ip_addr = IpAddr::V4(Ipv4Addr::from(ntohl(peer.sin_addr.s_addr)));
            let hostname = dns_lookup::lookup_addr(&ip_addr)
                .or::<std::io::Error>(Ok(ip_addr.to_string()))
                .expect("lookup with ip fallback to be infallible");

            Ok(Box::new(Descriptor {
                file_descriptor,
                hostname,
            }))
        }
    }
}

impl Drop for DescriptorManager {
    fn drop(&mut self) {
        unsafe {
            if close(self.socket) < 0 {
                todo!("handle socket closing failures");
            }
        }
    }
}

pub struct Descriptor {
    file_descriptor: c_int,
    hostname: String,
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe {
            if close(self.file_descriptor) < 0 {
                todo!("handle descriptor closing failures");
            }
        }
    }
}

impl Read for Descriptor {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        unsafe {
            // Check if there is anything to read
            let mut input_set: fd_set = mem::zeroed();
            let mut output_set: fd_set = mem::zeroed();
            let mut exc_set: fd_set = mem::zeroed();
            let mut timeout: timeval = mem::zeroed();
            FD_SET(self.file_descriptor, &mut input_set as *mut fd_set);
            if select(
                self.file_descriptor + 1,
                &mut input_set as *mut fd_set,
                &mut output_set as *mut fd_set,
                &mut exc_set as *mut fd_set,
                &mut timeout as *mut timeval,
            ) < 0
            {
                return Err(std::io::Error::new(ErrorKind::Other, "libc::select failed"));
            }

            if FD_ISSET(self.file_descriptor, &input_set as *const fd_set) {
                let retval = read(
                    self.file_descriptor,
                    buf.as_mut_ptr() as *mut c_void,
                    buf.len(),
                );
                if retval < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(retval as usize)
                }
            } else {
                Ok(0)
            }
        }
    }
}

impl Write for Descriptor {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        unsafe {
            let retval = send(
                self.file_descriptor,
                buf.as_ptr() as *const c_void,
                buf.len(),
                0,
            );
            if retval < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(retval as usize)
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // No meaningful on-demand flushing of unix network sockets (ie TCP_NODELAY is forever)
        Ok(())
    }
}
