use crate::{
    record::{layout::Layout, schema::Schema},
    scan::{table_scan::TableScan, Scan, UpdateScan},
    tx::transaction::Transaction,
};

const TABLE_NAME_MAX_LENGTH: usize = 50;
const FIELD_NAME_MAX_LENGTH: usize = 50;

pub struct TableManager {
    tcat_layout: Layout,
    fcat_layout: Layout,
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
    tx: &mut Transaction,
    tcat_layout: &Layout,
    fcat_layout: &Layout,
) -> Result<(), anyhow::Error> {
    let layout = Layout::new(schema.clone());
    {
        let mut tcat = TableScan::new(tx, "tblcat", tcat_layout)?;
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
            fcat.set_i32("type", layout.get_type_code(field_name))?;
            fcat.set_i32("length", layout.get_length(field_name) as i32)?;
            fcat.set_i32("offset", layout.get_offset(field_name) as i32)?;
        }
    }
    Ok(())
}

impl TableManager {
    pub fn new(is_new: bool, tx: &mut Transaction) -> Result<Self, anyhow::Error> {
        let tcat_layout = create_tcat_layout();
        let fcat_layout = create_fcat_layout();
        if is_new {
            create_table(
                "tblcat",
                &tcat_layout.schema,
                tx,
                &tcat_layout,
                &fcat_layout,
            )?;

            create_table(
                "fldcat",
                &fcat_layout.schema,
                tx,
                &tcat_layout,
                &fcat_layout,
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
        tx: &mut Transaction,
    ) -> Result<(), anyhow::Error> {
        create_table(
            table_name,
            &schema,
            tx,
            &self.tcat_layout,
            &self.fcat_layout,
        )
    }

    pub fn get_layout(
        &self,
        table_name: &str,
        tx: &mut Transaction,
    ) -> Result<Option<Layout>, anyhow::Error> {
        let mut schema = Schema::new();
        let mut fcat = TableScan::new(tx, "fldcat", &self.fcat_layout)?;
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
    fn test_table_manager() -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;
        let mut tx = db.new_transaction()?;
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);

        let table_manager = TableManager::new(true, &mut tx)?;
        table_manager.create_table("testtable", &schema, &mut tx)?;
        let layout = table_manager.get_layout("testtable", &mut tx)?.unwrap();
        assert_eq!(schema, layout.schema);
        tx.commit()?;
        Ok(())
    }
}
