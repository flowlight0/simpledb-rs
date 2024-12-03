use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    record::{layout::Layout, schema::Schema},
    scan::{table_scan::TableScan, Scan},
    tx::transaction::Transaction,
};

const TABLE_NAME_MAX_LENGTH: usize = 50;
const FIELD_NAME_MAX_LENGTH: usize = 50;

#[derive(Clone)]
pub struct TableManager {
    tcat_layout: Arc<Layout>,
    fcat_layout: Arc<Layout>,
}

fn create_tcat_layout() -> Layout {
    let mut schema = Schema::new();
    schema.add_string_field("tblname", TABLE_NAME_MAX_LENGTH);
    schema.add_i32_field("slotsize");
    Layout::new(schema)
}

fn create_fcat_layout() -> Layout {
    let mut schema = Schema::new();
    schema.add_string_field("tblname", TABLE_NAME_MAX_LENGTH);
    schema.add_string_field("fldname", FIELD_NAME_MAX_LENGTH);
    schema.add_i32_field("type");
    schema.add_i32_field("length");
    schema.add_i32_field("offset");
    Layout::new(schema)
}

fn create_table(
    table_name: &str,
    schema: &Schema,
    tx: Arc<Mutex<Transaction>>,
    tcat_layout: Arc<Layout>,
    fcat_layout: Arc<Layout>,
) -> Result<(), TransactionError> {
    let layout = Layout::new(schema.clone());
    {
        let mut tcat = TableScan::new(tx.clone(), "tblcat", tcat_layout)?;
        tcat.insert()?;
        tcat.set_string("tblname", table_name)?;
        tcat.set_i32("slotsize", layout.slot_size as i32)?;
        tcat.close()?;
    }

    {
        let mut fcat = TableScan::new(tx, "fldcat", fcat_layout)?;
        for field_name in layout
            .schema
            .i32_fields
            .iter()
            .chain(layout.schema.string_fields.iter())
        {
            fcat.insert()?;
            fcat.set_string("tblname", table_name)?;
            fcat.set_string("fldname", field_name)?;
            fcat.set_i32("type", layout.get_type_code(field_name))?;
            fcat.set_i32("length", layout.get_length(field_name) as i32)?;
            fcat.set_i32("offset", layout.get_offset(field_name) as i32)?;
        }
        fcat.close()?;
    }
    Ok(())
}

impl TableManager {
    pub fn new(is_new: bool, tx: Arc<Mutex<Transaction>>) -> Result<Self, TransactionError> {
        let tcat_layout = Arc::new(create_tcat_layout());
        let fcat_layout = Arc::new(create_fcat_layout());
        if is_new {
            create_table(
                "tblcat",
                &tcat_layout.schema,
                tx.clone(),
                tcat_layout.clone(),
                fcat_layout.clone(),
            )?;

            create_table(
                "fldcat",
                &fcat_layout.schema,
                tx,
                tcat_layout.clone(),
                fcat_layout.clone(),
            )?;
        }
        Ok(Self {
            tcat_layout,
            fcat_layout,
        })
    }

    pub fn create_table(
        &self,
        table_name: &str,
        schema: &Schema,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<(), TransactionError> {
        create_table(
            table_name,
            &schema,
            tx,
            self.tcat_layout.clone(),
            self.fcat_layout.clone(),
        )
    }

    pub fn get_layout(
        &self,
        table_name: &str,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<Option<Layout>, TransactionError> {
        let mut schema = Schema::new();
        let mut fcat = TableScan::new(tx, "fldcat", self.fcat_layout.clone())?;
        while fcat.next()? {
            if fcat.get_string("tblname")? != table_name {
                continue;
            }
            let field_name = fcat.get_string("fldname")?;
            let type_code = fcat.get_i32("type")?;
            let length = fcat.get_i32("length")?;
            // let offset = fcat.get_i32("offset")?;
            match type_code {
                0 => schema.add_i32_field(&field_name),
                1 => schema.add_string_field(&field_name, length as usize),
                _ => panic!("Unknown type code: {}", type_code),
            }
        }
        fcat.close()?;

        if schema.i32_fields.is_empty() && schema.string_fields.is_empty() {
            return Ok(None);
        }
        return Ok(Some(Layout::new(schema)));
    }
}

#[cfg(test)]
mod tests {
    use crate::db::SimpleDB;

    use super::*;

    #[test]
    fn test_table_manager() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);

        let table_manager = TableManager::new(true, tx.clone())?;
        table_manager.create_table("testtable", &schema, tx.clone())?;
        let layout = table_manager.get_layout("testtable", tx.clone())?.unwrap();
        assert_eq!(schema, layout.schema);
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
