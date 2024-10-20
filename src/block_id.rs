pub struct BlockId<'a> {
    pub file_name: &'a str,
    pub block_slot: usize,
}

impl<'a> BlockId<'a> {
    pub fn new(file_name: &'a str, block_slot: usize) -> Self {
        BlockId {
            file_name,
            block_slot,
        }
    }
}
