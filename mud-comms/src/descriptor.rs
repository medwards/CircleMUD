use std::{io::Read, io::Write};

pub trait DescriptorManager {
    fn block_until_descriptor(&self) -> Result<(), std::io::Error>;
    fn new_descriptor(
        &self,
    ) -> Result<Box<dyn Descriptor>, Box<dyn std::error::Error + Send + Sync>>;
}

pub trait Descriptor: Read + Write {
    fn get_hostname(&self) -> &str;
}
