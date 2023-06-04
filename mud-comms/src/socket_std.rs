use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;

use dns_lookup::lookup_addr;

use crate::descriptor::Descriptor;
use crate::descriptor::DescriptorManager;

pub struct SocketDescriptorManager {
    listener: TcpListener,
}

impl SocketDescriptorManager {
    pub fn new(listen_port: u16) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind(("0.0.0.0", listen_port))?;
        listener.set_nonblocking(true)?;
        Ok(SocketDescriptorManager { listener })
    }
}

impl DescriptorManager for SocketDescriptorManager {
    fn block_until_descriptor(&self) -> Result<(), std::io::Error> {
        Ok(())
    }

    fn new_descriptor(&self) -> Result<Box<dyn Descriptor>, std::io::Error> {
        self.listener.accept().and_then(
            |(stream, socket_addr)| -> Result<Box<dyn Descriptor>, std::io::Error> {
                if stream.set_nonblocking(true).is_err() {
                    stream.shutdown(std::net::Shutdown::Both)?;
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Couldn't set stream to nonblocking",
                    ));
                }
                let hostname = lookup_addr(&socket_addr.ip())
                    .or::<std::io::Error>(Ok(socket_addr.ip().to_string()))
                    .expect("lookup with ip fallback to be infallible");
                Ok(Box::new(SocketDescriptor { stream, hostname }))
            },
        )
    }
}

pub struct SocketDescriptor {
    stream: TcpStream,
    hostname: String,
}

impl Descriptor for SocketDescriptor {
    fn get_hostname(&self) -> &str {
        self.hostname.as_str()
    }
}

impl Read for SocketDescriptor {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.read(buf)
    }
}

impl Write for SocketDescriptor {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}
