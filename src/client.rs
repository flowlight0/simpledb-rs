use std::io::{stdout, Write};
use std::path::PathBuf;

use rustyline::{DefaultEditor, error::ReadlineError};

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

pub fn run_client<W: Write>(
    driver: Driver,
    editor: &mut DefaultEditor,
    writer: &mut W,
) -> Result<(), anyhow::Error> {
    let history_path = std::env::var("HOME")
        .map(PathBuf::from)
        .map(|p| p.join(".simpledb_history"))
        .ok();

    if let Some(ref path) = history_path {
        let _ = editor.load_history(path);
    }

    let db_url = match editor.readline("Connect> ") {
        Ok(line) => line,
        Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => return Ok(()),
        Err(e) => return Err(e.into()),
    };

    let (db_name, mut connection) = driver.connect(db_url.trim_end())?;
    let mut statement = connection.create_statement()?;

    loop {
        let prompt = format!("\nSQL ({})> ", db_name);
        let command = match editor.readline(&prompt) {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(e) => return Err(e.into()),
        };
        if command.starts_with("exit") {
            break;
        }

        let _ = editor.add_history_entry(command.as_str());

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
    }
    if let Some(ref path) = history_path {
        let _ = editor.save_history(path);
    }
    connection.commit()?;
    connection.close()?;
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    let mut editor = DefaultEditor::new()?;
    run_client(
        Driver::Embedded(EmbeddedDriver::new()),
        &mut editor,
        &mut stdout(),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::process::{Command, Stdio};

    #[test]
    fn test_run_client_select() -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir()?.into_path().join("directory");
        let db_url = format!("jdbc:simpledb:{}", temp_dir.to_string_lossy());
        let script = format!(
            "{db_url}\ncreate table T(A I32)\ninsert into T(A) values (1)\nselect A from T\nselect A from T\nexit\n",
        );

        let home_dir = tempfile::tempdir()?;
        std::env::set_var("HOME", home_dir.path());

        let mut exe_path = std::env::current_exe()?;
        exe_path.pop();
        exe_path.pop();
        exe_path.push("client");
        let mut child = Command::new(exe_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn client");
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(script.as_bytes())?;
        let output = child.wait_with_output()?;
        assert!(output.status.success());

        let output_str = String::from_utf8(output.stdout).unwrap();
        let expected = "0 records processed\n1 records processed\n           A\n------------\n           1\n           A\n------------\n           1\n";
        assert_eq!(output_str, expected);

        let history_file = home_dir.path().join(".simpledb_history");
        let history_content = fs::read_to_string(history_file)?;
        assert!(history_content.contains("select A from T"));
        Ok(())
    }
}
