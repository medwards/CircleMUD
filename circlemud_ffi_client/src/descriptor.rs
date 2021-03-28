use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::Mutex;
pub type ErrorCode = usize;

pub trait DescriptorManager {
    fn get_new_descriptor(&mut self) -> Option<DescriptorId>;
    fn get_descriptor(&mut self, descriptor: &DescriptorId) -> Option<&mut Box<dyn Descriptor>>;
    fn close_descriptor(&mut self, identifier: &DescriptorId);
}

// TODO: this should probably just require the Read trait
pub trait Descriptor: Send + Sync {
    fn identifier(&self) -> &DescriptorId;
    fn read(&mut self, read_point: &mut [u8]) -> Result<usize, ErrorCode>;
    fn write(&mut self, content: String) -> Result<usize, ErrorCode>;
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

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, ErrorCode> {
        // TODO: don't silently drop the error
        self.reader.read(buf).map_err(|e| 0)
    }

    fn write(&mut self, content: String) -> Result<usize, ErrorCode> {
        self.writer.write(content.as_bytes()).map_err(|e| 1)
    }
}
