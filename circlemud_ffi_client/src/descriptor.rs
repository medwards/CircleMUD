use std::io::{Read, Write};
pub type ErrorCode = usize;
pub type DescriptorId = String;

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
