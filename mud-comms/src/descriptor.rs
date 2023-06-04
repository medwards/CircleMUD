use std::{io::Read, io::Write};

pub trait DescriptorManager {
    fn block_until_descriptor(&self) -> Result<(), std::io::Error>;
    fn new_descriptor(&self) -> Result<Box<dyn Descriptor>, std::io::Error>;
}

pub trait Descriptor: Read + Write {
    fn get_hostname(&self) -> &str;
}
