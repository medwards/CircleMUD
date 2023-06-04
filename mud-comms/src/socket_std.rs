use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::thread::JoinHandle;

use crossbeam_channel::Receiver;
use dns_lookup::lookup_addr;

use crate::descriptor::Descriptor;
use crate::descriptor::DescriptorManager;

pub struct SocketDescriptorManager {
    listener_thread: JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    stream_receiver: Receiver<TcpStream>,
    descriptors_waiting_condition: Arc<(Condvar, Mutex<bool>)>,
}

impl SocketDescriptorManager {
    pub fn new(listen_port: u16) -> Result<Self, std::io::Error> {
        let (sender, receiver) = crossbeam_channel::bounded(5);
        let conditional_and_predicate = Arc::new((Condvar::new(), Mutex::new(true)));
        let sender_conditional_and_predicate = Arc::clone(&conditional_and_predicate);

        let listener_thread = std::thread::spawn(move || {
            let listener = TcpListener::bind(("0.0.0.0", listen_port))?;
            let (condition, predicate) = &*sender_conditional_and_predicate;

            for connection in listener.incoming() {
                match connection {
                    Ok(stream) => {
                        sender.send(stream)?;
                        let mut empty = predicate.lock().unwrap();
                        *empty = sender.is_empty();
                        condition.notify_one();
                    }
                    Err(e) => (),
                }
            }
            Ok(())
        });

        Ok(SocketDescriptorManager {
            listener_thread,
            stream_receiver: receiver,
            descriptors_waiting_condition: conditional_and_predicate,
        })
    }
}

impl DescriptorManager for SocketDescriptorManager {
    fn block_until_descriptor(&self) -> Result<(), std::io::Error> {
        let (condition, predicate) = &*self.descriptors_waiting_condition;
        let empty = predicate.lock().unwrap();
        if *empty {
            // subject to spurious unwraps but this is called in a loop anyways
            condition.wait(empty).unwrap();
        }
        Ok(())
    }

    fn new_descriptor(
        &self,
    ) -> Result<Box<dyn Descriptor>, Box<dyn std::error::Error + Send + Sync>> {
        match self.stream_receiver.try_recv() {
            Ok(stream) => {
                let (condition, predicate) = &*self.descriptors_waiting_condition;
                let mut empty = predicate.lock().unwrap();
                *empty = self.stream_receiver.is_empty();
                condition.notify_one();

                if stream.set_nonblocking(true).is_err() {
                    stream.shutdown(std::net::Shutdown::Both)?;
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Couldn't set stream to nonblocking",
                    )));
                }

                let socket_addr = stream.peer_addr()?;
                let hostname = lookup_addr(&socket_addr.ip())
                    .or::<std::io::Error>(Ok(socket_addr.ip().to_string()))
                    .expect("lookup with ip fallback to be infallible");
                Ok(Box::new(SocketDescriptor { stream, hostname }))
            }
            Err(e) => Err(Box::new(e)),
        }
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
