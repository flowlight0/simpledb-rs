use std::{
    fs::File,
    io::{Read, Write},
};

pub struct Page {
    byte_buffer: Vec<u8>,
}

impl Page {
    pub fn new(block_size: usize) -> Self {
        Page {
            byte_buffer: vec![0; block_size],
        }
    }

    pub fn set_i32(&mut self, offset: usize, value: i32) -> () {
        let bytes = value.to_le_bytes();
        self.byte_buffer[offset..offset + 4].copy_from_slice(&bytes);
    }

    pub fn get_i32(&self, offset: usize) -> i32 {
        i32::from_le_bytes([
            self.byte_buffer[offset],
            self.byte_buffer[offset + 1],
            self.byte_buffer[offset + 2],
            self.byte_buffer[offset + 3],
        ])
    }

    pub fn set_string(&mut self, offset: usize, string: &str) -> usize {
        self.set_bytes(offset, string.as_bytes())
    }

    pub fn get_string(&self, offset: usize) -> (&str, usize) {
        let (bytes, length) = self.get_bytes(offset);
        (std::str::from_utf8(bytes).unwrap(), length)
    }

    pub fn set_bytes(&mut self, offset: usize, bytes: &[u8]) -> usize {
        assert!(bytes.len() < (1usize << 16));
        // Use 2 bytes to store the length of the bytes.
        self.byte_buffer[offset..offset + 2].copy_from_slice(&(bytes.len() as u16).to_le_bytes());
        self.byte_buffer[offset + 2..offset + 2 + bytes.len()].copy_from_slice(bytes);
        bytes.len() + 2
    }

    pub fn get_bytes(&self, offset: usize) -> (&[u8], usize) {
        let length =
            u16::from_le_bytes([self.byte_buffer[offset], self.byte_buffer[offset + 1]]) as usize;
        (
            &self.byte_buffer[offset + 2..offset + 2 + length],
            length + 2,
        )
    }

    pub fn write_to_file(&self, file: &mut File) -> Result<(), std::io::Error> {
        file.write(self.byte_buffer.as_slice())?;
        Ok(())
    }

    pub fn read_from_file(&mut self, file: &mut File) -> Result<(), std::io::Error> {
        file.read_exact(&mut self.byte_buffer)?;
        Ok(())
    }

    pub fn get_required_size(&self, bytes: &[u8]) -> usize {
        assert!(bytes.len() < (1usize << 16));
        bytes.len() + 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_i32() {
        let mut page = Page::new(1024);
        page.set_i32(100, 1019);
        assert_eq!(page.get_i32(100), 1019);
    }

    #[test]
    fn test_set_string() {
        let mut page = Page::new(1024);
        let length = page.set_string(100, "Hello, world!");
        assert_eq!(page.get_string(100), ("Hello, world!", length));
    }
}
