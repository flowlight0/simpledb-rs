use super::Scan;

pub struct ProjectScan<'a> {
    base_scan: &'a mut dyn Scan,
    fields: Vec<String>,
}

impl<'a> ProjectScan<'a> {
    pub fn new(base_scan: &'a mut dyn Scan, fields: Vec<String>) -> Self {
        for field in &fields {
            assert!(base_scan.has_field(field));
        }

        ProjectScan { base_scan, fields }
    }
}

impl<'a> Scan for ProjectScan<'a> {
    fn before_first(&mut self) -> Result<(), anyhow::Error> {
        self.base_scan.before_first()
    }

    fn next(&mut self) -> Result<bool, anyhow::Error> {
        self.base_scan.next()
    }

    fn get_i32(&mut self, field_name: &str) -> Result<i32, anyhow::Error> {
        assert!(self.fields.contains(&field_name.to_string()));
        self.base_scan.get_i32(field_name)
    }

    fn get_string(&mut self, field_name: &str) -> Result<String, anyhow::Error> {
        assert!(self.fields.contains(&field_name.to_string()));
        self.base_scan.get_string(field_name)
    }

    fn has_field(&self, field_name: &str) -> bool {
        self.fields.contains(&field_name.to_string())
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        db::SimpleDB,
        record::{layout::Layout, schema::Schema},
        scan::table_scan::TableScan,
    };

    use super::*;

    #[test]
    fn test_project_scan() -> Result<(), anyhow::Error> {
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);
        schema.add_i32_field("C");
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
            table_scan.set_i32("C", i + 2)?;
        }

        let mut project_scan =
            ProjectScan::new(&mut table_scan, vec!["A".to_string(), "C".to_string()]);
        project_scan.before_first()?;
        for i in 0..50 {
            assert!(project_scan.next()?);
            assert!(project_scan.has_field("A"));
            assert!(!project_scan.has_field("B"));
            assert!(project_scan.has_field("C"));
            assert_eq!(project_scan.get_i32("A")?, i);
            assert_eq!(project_scan.get_i32("C")?, i + 2);
        }
        drop(project_scan);
        drop(table_scan);
        tx.commit()?;
        Ok(())
    }
}