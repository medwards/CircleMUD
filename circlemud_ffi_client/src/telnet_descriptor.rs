use std::collections::HashMap;
use std::io::{ErrorKind, Read, Result as IoResult, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;

use log::*;

use crate::descriptor;
use crate::descriptor::Descriptor;

pub struct TelnetDescriptorManager {
    server: thread::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    descriptors: HashMap<String, Box<dyn descriptor::Descriptor>>,
    new_connections: mpsc::Receiver<TcpStream>,
}

impl TelnetDescriptorManager {
    pub fn new() -> Self {
        let (new_connections_send, new_connections_recv) = mpsc::channel();
        let server = thread::spawn(move || {
            info!("Launching telnet listener");
            let listener = TcpListener::bind("127.0.0.1:2323")?;

            for conn in listener.incoming() {
                match conn {
                    Ok(stream) => new_connections_send.send(stream)?,
                    Err(e) => error!("Failed connection: {}", e),
                }
            }
            Ok(())
        });
        Self {
            server,
            descriptors: HashMap::new(),
            new_connections: new_connections_recv,
        }
    }
}

impl descriptor::DescriptorManager for TelnetDescriptorManager {
    fn get_new_descriptor(&mut self) -> Option<descriptor::DescriptorId> {
        // see if there are any new descriptors and return them
        match self.new_connections.try_recv() {
            Ok(stream) => {
                let identifier = descriptor::DescriptorId::new(
                    format!("{:?}", stream.peer_addr()).as_ref(),
                    "telnet",
                );
                stream.set_nonblocking(true);
                let descriptor = TelnetDescriptor {
                    identifier: identifier.clone(),
                    stream,
                };
                self.descriptors
                    .insert(identifier.identifier.clone(), Box::new(descriptor));
                Some(identifier)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(_) => panic!("Channel for receiving new SlackDescriptors unexpectedly closed"),
        }
    }

    fn get_descriptor(
        &mut self,
        descriptor: &descriptor::DescriptorId,
    ) -> Option<&mut Box<dyn Descriptor>> {
        self.descriptors.get_mut(&descriptor.identifier)
    }

    fn close_descriptor(&mut self, identifier: &descriptor::DescriptorId) {
        self.descriptors.remove(&identifier.identifier);
    }
}

struct TelnetDescriptor {
    stream: TcpStream,
    identifier: descriptor::DescriptorId,
}

impl Descriptor for TelnetDescriptor {
    fn identifier(&self) -> &descriptor::DescriptorId {
        &self.identifier
    }
}

impl Read for TelnetDescriptor {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.stream.read(buf).or_else(|e| {
            if e.kind() == ErrorKind::WouldBlock {
                Ok(0)
            } else {
                Err(e)
            }
        })
    }
}

impl Write for TelnetDescriptor {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.stream.write(buf).or_else(|e| {
            if e.kind() == ErrorKind::WouldBlock {
                Ok(0)
            } else {
                Err(e)
            }
        })
    }

    fn flush(&mut self) -> IoResult<()> {
        self.stream.flush()
    }
}
