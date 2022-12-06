use std::io::{Read, Write};
use bytemuck::Pod;

pub trait WriteUtil{
    fn write_pod<T: Pod>(&mut self, thing: &T) -> std::io::Result<()>;
}

pub trait ReadUtil{
    fn read_pod<T: Pod>(&mut self) -> std::io::Result<T>;
}

impl<W: Write> WriteUtil for W{
    fn write_pod<T: Pod>(&mut self, thing: &T) -> std::io::Result<()>{
        let bytes = bytemuck::bytes_of(thing);
        self.write_all(&bytes)
    }
}

impl<R: Read> ReadUtil for R{
    fn read_pod<T: Pod>(&mut self) -> std::io::Result<T>{
        let mut result = T::zeroed();
        self.read_exact(bytemuck::bytes_of_mut(&mut result))?;
        Ok(result)
    }
}