use crate::{file::BlockId, record::layout::Layout, tx::transaction::Transaction};

use super::record_page::{RecordPage, Slot};

pub struct TableScan<'a> {
    file_name: String,
    record_page: RecordPage<'a>,
    current_slot: Slot,
}

impl<'a> TableScan<'a> {
    pub fn new(
        tx: &'a mut Transaction,
        table_name: &str,
        layout: &'a Layout,
    ) -> Result<Self, anyhow::Error> {
        let file_name = format!("{}.tbl", table_name);
        let record_page = {
            let (block, is_new) = if tx.get_num_blocks(&file_name)? == 0 {
                (tx.append_block(&file_name)?, true)
            } else {
                (BlockId::get_first_block(&file_name), false)
            };

            tx.pin(&block)?;
            let mut rp = RecordPage::new(tx, block, layout);
            if is_new {
                rp.format()?;
            }
            rp
        };

        Ok(Self {
            file_name,
            record_page,
            current_slot: Slot::Start,
        })
    }

    pub fn before_first(&mut self) -> Result<(), anyhow::Error> {
        let block = BlockId::get_first_block(&self.file_name);
        self.record_page.tx.unpin(&self.record_page.block);
        self.record_page.tx.pin(&block)?;
        self.record_page.reset_block(block);
        self.current_slot = Slot::Start;
        Ok(())
    }

    pub fn next(&mut self) -> Result<bool, anyhow::Error> {
        loop {
            self.current_slot = self.record_page.next_after(self.current_slot)?;
            match self.current_slot {
                Slot::Index(_) => return Ok(true),
                Slot::Start => unreachable!(),
                Slot::End => {
                    if self.record_page.tx.is_last_block(&self.record_page.block)? {
                        return Ok(false);
                    } else {
                        let new_block = self.record_page.block.get_next_block();
                        self.record_page.tx.unpin(&self.record_page.block);
                        self.record_page.tx.pin(&new_block)?;
                        self.record_page.reset_block(new_block);
                        self.current_slot = Slot::Start;
                    }
                }
            }
        }
    }

    pub fn insert(&mut self) -> Result<(), anyhow::Error> {
        loop {
            self.current_slot = self.record_page.insert_after(self.current_slot)?;
            match self.current_slot {
                Slot::Index(_) => return Ok(()),
                Slot::Start => unreachable!(),
                Slot::End => {
                    let next_block =
                        if self.record_page.tx.is_last_block(&self.record_page.block)? {
                            self.record_page.tx.append_block(&self.file_name)?
                        } else {
                            self.record_page.block.get_next_block()
                        };

                    self.record_page.tx.unpin(&self.record_page.block);
                    self.record_page.tx.pin(&next_block)?;
                    self.record_page.reset_block(next_block);
                    self.current_slot = Slot::Start;
                }
            }
        }
    }

    pub fn get_i32(&mut self, field_name: &str) -> Result<i32, anyhow::Error> {
        self.record_page
            .get_i32(self.current_slot.get_index(), field_name)
    }

    pub fn set_i32(&mut self, field_name: &str, value: i32) -> Result<(), anyhow::Error> {
        self.record_page
            .set_i32(self.current_slot.get_index(), field_name, value)
    }

    pub fn get_string(&mut self, field_name: &str) -> Result<String, anyhow::Error> {
        self.record_page
            .get_string(self.current_slot.get_index(), field_name)
    }

    pub fn set_string(&mut self, field_name: &str, value: &str) -> Result<(), anyhow::Error> {
        self.record_page
            .set_string(self.current_slot.get_index(), field_name, value)
    }

    pub fn delete(&mut self) -> Result<(), anyhow::Error> {
        self.record_page.delete(self.current_slot.get_index())
    }
}

impl Drop for TableScan<'_> {
    fn drop(&mut self) {
        self.record_page.tx.unpin(&self.record_page.block);
    }
}

#[cfg(test)]
mod tests {

    use crate::{db::SimpleDB, record::schema::Schema};

    use super::*;

    #[test]
    fn test_table_scan() -> Result<(), anyhow::Error> {
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);
        let layout = Layout::new(schema);

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;

        let mut tx = db.new_transaction()?;

        let mut table_scan = TableScan::new(&mut tx, "testtable", &layout)?;
        table_scan.before_first()?;
        for i in 0..50 {
            table_scan.insert()?;
            table_scan.set_i32("A", i)?;
            table_scan.set_string("B", &i.to_string())?;

            assert_eq!(table_scan.get_i32("A")?, i);
            assert_eq!(table_scan.get_string("B")?, i.to_string());
            let value = table_scan
                .record_page
                .tx
                .get_i32(&table_scan.record_page.block, 0)?;
            assert_eq!(value, 1);
        }

        table_scan.before_first()?;
        let value = table_scan
            .record_page
            .tx
            .get_i32(&table_scan.record_page.block, 0)?;
        assert_eq!(value, 1);

        let mut num_scanned_records = 0;
        let mut num_deleted_records = 0;
        while table_scan.next()? {
            let value = table_scan.get_i32("A")?;
            num_scanned_records += 1;
            if value % 2 == 0 {
                table_scan.delete()?;
                num_deleted_records += 1;
            }
        }
        assert_eq!(num_scanned_records, 50);
        assert_eq!(num_deleted_records, 25);

        table_scan.before_first()?;
        for i in 0..25 {
            assert!(table_scan.next()?);
            let i32_value = table_scan.get_i32("A")?;
            assert_eq!(i32_value, i * 2 + 1);
            let string_value = table_scan.get_string("B")?;
            assert_eq!(string_value, (i * 2 + 1).to_string());
        }
        assert!(!table_scan.next()?);
        drop(table_scan);
        tx.commit()?;
        Ok(())
    }
}
