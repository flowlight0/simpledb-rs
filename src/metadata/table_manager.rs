use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    record::{field::Type, layout::Layout, schema::Schema},
    scan::{table_scan::TableScan, ScanControl},
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
            let field_type = layout.schema.get_field_type(field_name);
            fcat.set_i32("type", field_type.to_code())?;
            fcat.set_i32("length", layout.get_length(field_name) as i32)?;
            fcat.set_i32("offset", layout.get_offset(field_name) as i32)?;
        }
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
        if self.get_layout(table_name, tx.clone())?.is_some() {
            return Err(TransactionError::TableAlreadyExists(table_name.to_string()));
        }
        create_table(
            table_name,
            &schema,
            tx,
            self.tcat_layout.clone(),
            self.fcat_layout.clone(),
        )
    }

    // Get the layout of a table
    // If the table does not exist, return None
    pub fn get_layout(
        &self,
        table_name: &str,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<Option<Layout>, TransactionError> {
        let mut schema = Schema::new();
        let mut fcat = TableScan::new(tx, "fldcat", self.fcat_layout.clone())?;
        while fcat.next()? {
            if fcat.get_string("tblname")? != Some(table_name.to_string()) {
                continue;
            }
            let field_name = fcat.get_string("fldname")?.unwrap();
            let type_code = fcat.get_i32("type")?;
            let length = fcat.get_i32("length")?;
            let field_type = match type_code {
                Some(code) => Type::try_from(code).unwrap(),
                None => panic!("Missing type code for field {}", field_name),
            };
            match field_type {
                Type::I32 => schema.add_i32_field(&field_name),
                Type::String => schema.add_string_field(&field_name, length.unwrap() as usize),
            }
        }
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

    #[test]
    fn test_create_table_conflict() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let mut schema = Schema::new();
        schema.add_i32_field("A");

        let table_manager = TableManager::new(true, tx.clone())?;
        table_manager.create_table("testtable", &schema, tx.clone())?;
        let err = table_manager.create_table("testtable", &schema, tx.clone());
        assert!(matches!(err, Err(TransactionError::TableAlreadyExists(_))));
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
