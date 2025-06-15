use std::io::{stdin, stdout, Write, BufRead};

use simpledb_rs::{
    driver::{
        embedded::EmbeddedDriver, ConnectionControl, Driver, DriverControl, MetadataControl,
        ResultSetControl, Statement, StatementControl,
    },
    record::field::Type,
};

fn do_query<W: Write>(
    statement: &mut Statement,
    command: &str,
    writer: &mut W,
) -> Result<(), anyhow::Error> {
    let mut result_set = statement.execute_query(command)?;
    let metadata = result_set.get_metadata()?;
    let num_columns = metadata.get_column_count()?;
    let mut total_width = 0;

    // print header
    for i in 0..num_columns {
        let column_name = metadata.get_column_name(i)?;
        let mut width = metadata.get_column_display_size(i)?;
        if i > 0 {
            width += 1;
        }

        total_width += width;
        write!(writer, "{:>width$}", column_name, width = width)?;
    }
    writeln!(writer)?;
    for _ in 0..total_width {
        write!(writer, "-")?;
    }
    writeln!(writer)?;

    // print records
    while result_set.next()? {
        for i in 0..num_columns {
            let column_name = metadata.get_column_name(i)?;
            let column_type = metadata.get_column_type(i)?;
            let width = metadata.get_column_display_size(i)?;
            // String fmt = "%" + md.getColumnDisplaySize(i);
            if i > 0 {
                write!(writer, " ")?;
            }

            match column_type {
                Type::I32 => {
                    let val = result_set.get_i32(&column_name)?;
                    write!(writer, "{:>width$}", val, width = width)?;
                }
                Type::String => {
                    let val = result_set.get_string(&column_name)?;
                    write!(writer, "{:>width$}", val, width = width)?;
                }
            }
        }
        writeln!(writer)?;
    }
    result_set.close()?;
    Ok(())
}

fn do_update<W: Write>(
    statement: &mut Statement,
    command: &str,
    writer: &mut W,
) -> Result<(), anyhow::Error> {
    let num_records = statement.execute_update(command)?;
    writeln!(writer, "{} records processed", num_records)?;
    Ok(())
}

pub fn run_client<R: BufRead, W: Write>(
    driver: Driver,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), anyhow::Error> {
    write!(writer, "Connect> ")?;
    writer.flush()?;
    let mut db_url = String::new();
    reader.read_line(&mut db_url)?;

    let (db_name, mut connection) = driver.connect(db_url.trim_end())?;
    let mut statement = connection.create_statement()?;

    write!(writer, "\nSQL ({})> ", db_name)?;
    writer.flush()?;
    loop {
        let mut command = String::new();
        if reader.read_line(&mut command)? == 0 {
            break;
        }
        if command.starts_with("exit") {
            break;
        }

        let trimmed = command.trim_start();
        let result = if trimmed.to_ascii_uppercase().starts_with("SELECT") {
            do_query(&mut statement, &command, writer)
        } else {
            do_update(&mut statement, &command, writer)
        };

        if let Err(e) = result {
            eprintln!("Error: {}", e);
            connection.rollback()?;
        }
        write!(writer, "\nSQL ({})> ", db_name)?;
        writer.flush()?;
    }
    connection.commit()?;
    connection.close()?;
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    run_client(
        Driver::Embedded(EmbeddedDriver::new()),
        &mut stdin().lock(),
        &mut stdout(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    // Cursor provides an in-memory buffer that implements `BufRead` and `Write`.
    use std::io::Cursor;

    #[test]
    fn test_run_client_select() -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir()?.into_path().join("directory");
        let db_url = format!("jdbc:simpledb:{}", temp_dir.to_string_lossy());

        let script = format!(
            "{db_url}\ncreate table T(A I32)\ninsert into T(A) values (1)\nselect A from T\nexit\n"
        );

        let mut reader = Cursor::new(script.clone().into_bytes());
        let mut output = Vec::new();
        run_client(Driver::Embedded(EmbeddedDriver::new()), &mut reader, &mut output)?;

        let output_str = String::from_utf8(output).unwrap();
        let db_name = db_url.trim_start_matches("jdbc:simpledb:");
        let expected = format!(
            "Connect> \nSQL ({db_name})> 0 records processed\n\n\
SQL ({db_name})> 1 records processed\n\n\
SQL ({db_name})>            A\n------------\n           1\n\n\
SQL ({db_name})> "
        );
        assert_eq!(output_str, expected);
        Ok(())
    }
}
