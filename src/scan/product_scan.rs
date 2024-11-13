use super::Scan;

pub struct ProductScan<'a> {
    scan1: &'a mut dyn Scan,
    scan2: &'a mut dyn Scan,
}

impl<'a> ProductScan<'a> {
    pub fn new(scan1: &'a mut dyn Scan, scan2: &'a mut dyn Scan) -> Self {
        ProductScan { scan1, scan2 }
    }
}

impl<'a> Scan for ProductScan<'a> {
    fn before_first(&mut self) -> Result<(), anyhow::Error> {
        self.scan1.before_first()?;
        self.scan1.next()?;
        self.scan2.before_first()
    }

    fn next(&mut self) -> Result<bool, anyhow::Error> {
        if self.scan2.next()? {
            Ok(true)
        } else {
            self.scan2.before_first()?;
            Ok(self.scan1.next()? && self.scan2.next()?)
        }
    }

    fn get_i32(&mut self, field_name: &str) -> Result<i32, anyhow::Error> {
        if self.scan1.has_field(field_name) {
            self.scan1.get_i32(field_name)
        } else {
            self.scan2.get_i32(field_name)
        }
    }

    fn get_string(&mut self, field_name: &str) -> Result<String, anyhow::Error> {
        if self.scan1.has_field(field_name) {
            self.scan1.get_string(field_name)
        } else {
            self.scan2.get_string(field_name)
        }
    }

    fn has_field(&self, field_name: &str) -> bool {
        self.scan1.has_field(field_name) || self.scan2.has_field(field_name)
    }
}

#[cfg(test)]
mod tests {

    use std::{
        rc::Rc,
        sync::{Arc, Mutex},
    };

    use crate::{
        db::SimpleDB,
        record::{layout::Layout, schema::Schema},
        scan::table_scan::TableScan,
    };

    use super::*;

    #[test]
    fn test_product_scan() -> Result<(), anyhow::Error> {
        let mut schema1 = Schema::new();
        schema1.add_i32_field("A");
        schema1.add_string_field("B", 20);
        schema1.add_i32_field("C");
        let layout1 = Rc::new(Layout::new(schema1));

        let mut schema2 = Schema::new();
        schema2.add_i32_field("D");
        schema2.add_string_field("E", 20);
        let layout2 = Rc::new(Layout::new(schema2));

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;

        let tx1 = Arc::new(Mutex::new(db.new_transaction()?));
        let mut scan1 = TableScan::new(tx1.clone(), "testtable1", layout1.clone())?;
        scan1.before_first()?;
        for i in 0..10 {
            scan1.insert()?;
            scan1.set_i32("A", i)?;
            scan1.set_string("B", &i.to_string())?;
            scan1.set_i32("C", i + 2)?;
        }

        let tx2 = Arc::new(Mutex::new(db.new_transaction()?));
        let mut scan2 = TableScan::new(tx2.clone(), "testtable2", layout2.clone())?;
        scan1.before_first()?;
        for i in 0..10 {
            scan2.insert()?;
            scan2.set_i32("D", i)?;
            scan2.set_string("E", &i.to_string())?;
        }

        let mut product_scan = ProductScan::new(&mut scan1, &mut scan2);
        product_scan.before_first()?;
        for i in 0..10 {
            for j in 0..10 {
                assert!(product_scan.next()?);
                assert_eq!(product_scan.get_i32("A")?, i);
                assert_eq!(product_scan.get_string("B")?, i.to_string());
                assert_eq!(product_scan.get_i32("C")?, i + 2);
                assert_eq!(product_scan.get_i32("D")?, j);
                assert_eq!(product_scan.get_string("E")?, j.to_string());
            }
        }
        assert!(!product_scan.next()?);
        drop(product_scan);

        drop(scan1);
        tx1.lock().unwrap().commit()?;

        drop(scan2);
        tx2.lock().unwrap().commit()?;
        Ok(())
    }
}
