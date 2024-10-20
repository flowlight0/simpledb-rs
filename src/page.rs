pub struct Page {
    pub byte_buffer: Vec<u8>,
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
}