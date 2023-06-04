use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::mem;
use std::mem::size_of;
use std::net::IpAddr;
use std::net::Ipv4Addr;

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

use crate::descriptor::Descriptor;
use crate::descriptor::DescriptorManager;

pub struct SocketDescriptorManager {
    pub(crate) socket: c_int,
}

// byte ordering helpers from sys::common::net
pub fn htons(u: u16) -> u16 {
    u.to_be()
}

pub fn htonl(u: u32) -> u32 {
    u.to_be()
}

pub fn ntohl(u: u32) -> u32 {
    u32::from_be(u)
}

impl SocketDescriptorManager {
    pub(crate) fn new(listen_port: u16) -> Result<SocketDescriptorManager, std::io::Error> {
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
            if bind(
                s,
                &addr_in as *const sockaddr_in as *const sockaddr,
                size_of::<sockaddr_in>() as u32,
            ) < 0
            {
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
            Ok(SocketDescriptorManager { socket: s })
        }
    }
}

impl DescriptorManager for SocketDescriptorManager {
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

    fn new_descriptor(
        &self,
    ) -> Result<Box<dyn Descriptor>, Box<dyn std::error::Error + Send + Sync>> {
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
                return Err(Box::new(std::io::Error::new(
                    ErrorKind::Other,
                    "libc::select failed",
                )));
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
                return Err(Box::new(std::io::Error::new(
                    ErrorKind::Other,
                    "libc::accept failed",
                )));
            }

            let ip_addr = IpAddr::V4(Ipv4Addr::from(ntohl(peer.sin_addr.s_addr)));
            let hostname = dns_lookup::lookup_addr(&ip_addr)
                .or::<std::io::Error>(Ok(ip_addr.to_string()))
                .expect("lookup with ip fallback to be infallible");

            Ok(Box::new(SocketDescriptor {
                file_descriptor,
                hostname,
            }))
        }
    }
}

impl Drop for SocketDescriptorManager {
    fn drop(&mut self) {
        unsafe {
            if close(self.socket) < 0 {
                todo!("handle socket closing failures");
            }
        }
    }
}

pub struct SocketDescriptor {
    pub(crate) file_descriptor: c_int,
    pub(crate) hostname: String,
}

impl Drop for SocketDescriptor {
    fn drop(&mut self) {
        unsafe {
            if close(self.file_descriptor) < 0 {
                todo!("handle descriptor closing failures");
            }
        }
    }
}

impl Descriptor for SocketDescriptor {
    fn get_hostname(&self) -> &str {
        self.hostname.as_str()
    }
}

impl Read for SocketDescriptor {
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

impl Write for SocketDescriptor {
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
