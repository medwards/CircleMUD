use std::io::{Read, Result as IoResult, Write};
use std::sync::Arc;
use std::sync::Mutex;
pub type ErrorCode = usize;

pub trait DescriptorManager {
    fn get_new_descriptor(&mut self) -> Option<DescriptorId>;
    fn get_descriptor(&mut self, descriptor: &DescriptorId) -> Option<&mut Box<dyn Descriptor>>;
    fn close_descriptor(&mut self, identifier: &DescriptorId);
}

pub trait Descriptor: Send + Sync + Read + Write {
    fn identifier(&self) -> &DescriptorId;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorId {
    pub identifier: String,
    pub descriptor_type: String,
}

impl DescriptorId {
    pub fn new(identifier: &str, descriptor_type: &str) -> Self {
        DescriptorId {
            identifier: identifier.to_owned(),
            descriptor_type: descriptor_type.to_owned(),
        }
    }
}

pub struct ByteStreamDescriptor {
    reader: Box<dyn Read + Send + Sync>,
    writer: Box<dyn Write + Send + Sync>,
}

impl ByteStreamDescriptor {
    pub fn new(
        reader: Box<dyn std::io::Read + Send + Sync + 'static>,
        writer: Box<dyn std::io::Write + Send + Sync + 'static>,
    ) -> Self {
        Self {
            reader: reader,
            writer: writer,
        }
    }
    // no special drop impl requited
}

impl Descriptor for ByteStreamDescriptor {
    fn identifier(&self) -> &DescriptorId {
        unimplemented!()
    }
}

impl Read for ByteStreamDescriptor {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.reader.read(buf)
    }
}

impl Write for ByteStreamDescriptor {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.writer.flush()
    }
}
