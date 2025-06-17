use std::{
    cmp::min,
    sync::{Arc, Mutex},
};

use crate::{
    errors::TransactionError,
    materialization::temp_table::TempTable,
    record::{field::Value, layout::Layout},
    scan::{product_scan::ProductScan, Scan, ScanControl},
    tx::transaction::Transaction,
};

use super::{buffer_needs, chunk_scan::ChunkScan};

pub struct MultiBufferProductScan {
    tx: Arc<Mutex<Transaction>>,
    product_scan: Option<ProductScan>,
    rhs_file_name: String,
    layout: Arc<Layout>,
    file_num_blocks: usize,
    chunk_size: usize,
    next_block_slot: usize,
}

impl MultiBufferProductScan {
    pub(crate) fn new(
        tx: Arc<Mutex<Transaction>>,
        lhs_scan: Scan,
        rhs_table_name: &str,
        layout: Arc<Layout>,
    ) -> Result<Self, TransactionError> {
        let rhs_file_name = format!("{}.tbl", rhs_table_name);
        let file_num_blocks = tx.lock().unwrap().get_num_blocks(&rhs_file_name)?;
        let num_available_buffers = tx.lock().unwrap().get_num_available_buffers();
        let chunk_size = buffer_needs::best_factor(num_available_buffers, file_num_blocks);
        let dummy_scan = TempTable::new(tx.clone(), &layout.schema).open()?;

        let mut scan = MultiBufferProductScan {
            tx,
            product_scan: Some(ProductScan::new(lhs_scan, Scan::from(dummy_scan))?),
            rhs_file_name,
            layout,
            file_num_blocks,
            chunk_size,
            next_block_slot: 0,
        };
        scan.before_first()?;
        Ok(scan)
    }

    fn use_next_chunk(&mut self) -> Result<bool, TransactionError> {
        if self.next_block_slot >= self.file_num_blocks {
            return Ok(false);
        }

        let next_chunk_block_start = self.next_block_slot;
        let next_chunk_block_end =
            min(self.next_block_slot + self.chunk_size, self.file_num_blocks);

        let rhs_scan = ChunkScan::new(
            self.tx.clone(),
            self.layout.clone(),
            &self.rhs_file_name,
            next_chunk_block_start,
            next_chunk_block_end,
        )?;

        let product_scan = self.product_scan.take().unwrap();
        let mut lhs_scan = *product_scan.scan1;
        lhs_scan.before_first()?;
        self.product_scan = Some(ProductScan::new(lhs_scan, Scan::from(rhs_scan))?);
        self.next_block_slot = next_chunk_block_end;
        Ok(true)
    }
}

impl ScanControl for MultiBufferProductScan {
    fn before_first(&mut self) -> Result<(), TransactionError> {
        self.next_block_slot = 0;
        self.use_next_chunk()?;

        Ok(())
    }

    fn after_last(&mut self) -> Result<(), TransactionError> {
        if self.file_num_blocks == 0 {
            return Ok(());
        }
        let remainder = self.file_num_blocks % self.chunk_size;
        let start = if remainder == 0 {
            self.file_num_blocks - self.chunk_size
        } else {
            self.file_num_blocks - remainder
        };
        self.next_block_slot = self.file_num_blocks;

        let rhs_scan = ChunkScan::new(
            self.tx.clone(),
            self.layout.clone(),
            &self.rhs_file_name,
            start,
            self.file_num_blocks,
        )?;
        let product_scan = self.product_scan.take().unwrap();
        let mut lhs_scan = *product_scan.scan1;
        lhs_scan.after_last()?;
        self.product_scan = Some(ProductScan::new(lhs_scan, Scan::from(rhs_scan))?);
        self.next_block_slot = start;
        self.product_scan.as_mut().unwrap().after_last()?;
        Ok(())
    }

    fn next(&mut self) -> Result<bool, TransactionError> {
        while !self.product_scan.as_mut().unwrap().next()? {
            if !self.use_next_chunk()? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn previous(&mut self) -> Result<bool, TransactionError> {
        while !self.product_scan.as_mut().unwrap().previous()? {
            if self.next_block_slot == 0 {
                return Ok(false);
            }
            let prev_chunk_end = self.next_block_slot;
            let prev_chunk_start = prev_chunk_end.saturating_sub(self.chunk_size);
            let rhs_scan = ChunkScan::new(
                self.tx.clone(),
                self.layout.clone(),
                &self.rhs_file_name,
                prev_chunk_start,
                prev_chunk_end,
            )?;
            let product_scan = self.product_scan.take().unwrap();
            let mut lhs_scan = *product_scan.scan1;
            lhs_scan.after_last()?;
            self.product_scan = Some(ProductScan::new(lhs_scan, Scan::from(rhs_scan))?);
            self.next_block_slot = prev_chunk_start;
            self.product_scan.as_mut().unwrap().after_last()?;
        }
        Ok(true)
    }

    fn get_i32(&mut self, field_name: &str) -> Result<i32, TransactionError> {
        self.product_scan.as_mut().unwrap().get_i32(field_name)
    }

    fn get_string(&mut self, field_name: &str) -> Result<String, TransactionError> {
        self.product_scan.as_mut().unwrap().get_string(field_name)
    }

    fn get_value(&mut self, field_name: &str) -> Result<Value, TransactionError> {
        self.product_scan.as_mut().unwrap().get_value(field_name)
    }

    fn has_field(&self, field_name: &str) -> bool {
        self.product_scan.as_ref().unwrap().has_field(field_name)
    }
}
