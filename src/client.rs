use std::io::{stdout, Write};
use std::path::{Path, PathBuf};

use colored::Colorize;

use rustyline::{error::ReadlineError, DefaultEditor};

use simpledb_rs::{
    driver::{
        embedded::EmbeddedDriver, ConnectionControl, Driver, DriverControl, MetadataControl,
        ResultSetControl, Statement, StatementControl,
    },
    record::field::Type,
};

trait ClientEditor {
    fn readline(&mut self, prompt: &str) -> Result<String, ReadlineError>;
    fn load_history<P: AsRef<Path> + ?Sized>(&mut self, path: &P) -> rustyline::Result<()>;
    fn save_history<P: AsRef<Path> + ?Sized>(&mut self, path: &P) -> rustyline::Result<()>;
    fn add_history_entry<S: AsRef<str> + Into<String>>(&mut self, line: S) -> rustyline::Result<bool>;
}

impl ClientEditor for DefaultEditor {
    fn readline(&mut self, prompt: &str) -> Result<String, ReadlineError> {
        Self::readline(self, prompt)
    }

    fn load_history<P: AsRef<Path> + ?Sized>(&mut self, path: &P) -> rustyline::Result<()> {
        Self::load_history(self, path)
    }

    fn save_history<P: AsRef<Path> + ?Sized>(&mut self, path: &P) -> rustyline::Result<()> {
        Self::save_history(self, path)
    }

    fn add_history_entry<S: AsRef<str> + Into<String>>(&mut self, line: S) -> rustyline::Result<bool> {
        Self::add_history_entry(self, line)
    }
}

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
        let header = format!("{:>width$}", column_name, width = width);
        write!(writer, "{}", header.bold().cyan())?;
    }
    writeln!(writer)?;
    let separator = "-".repeat(total_width);
    writeln!(writer, "{}", separator.bright_blue())?;

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
                    let v = format!("{:>width$}", val, width = width);
                    write!(writer, "{}", v.yellow())?;
                }
                Type::String => {
                    let val = result_set.get_string(&column_name)?;
                    let v = format!("{:>width$}", val, width = width);
                    write!(writer, "{}", v.green())?;
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
    let msg = format!("{} records processed", num_records);
    writeln!(writer, "{}", msg.magenta())?;
    Ok(())
}

pub fn run_client<W: Write, E: ClientEditor>(
    driver: Driver,
    editor: &mut E,
    writer: &mut W,
) -> Result<(), anyhow::Error> {
    let history_path = PathBuf::from(".simpledb_history");
    let _ = editor.load_history(&history_path);

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
    let _ = editor.save_history(&history_path);
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
    use super::{run_client, ClientEditor, ReadlineError};
    use std::fs;
    use std::path::Path;
    use simpledb_rs::driver::{Driver, embedded::EmbeddedDriver};

    struct ScriptedEditor {
        lines: Vec<String>,
        pos: usize,
        history: Vec<String>,
    }

    impl ScriptedEditor {
        fn new<I: Into<String>>(lines: Vec<I>) -> Self {
            Self { lines: lines.into_iter().map(Into::into).collect(), pos: 0, history: Vec::new() }
        }
    }

    impl ClientEditor for ScriptedEditor {
        fn readline(&mut self, _prompt: &str) -> Result<String, ReadlineError> {
            if self.pos >= self.lines.len() {
                Err(ReadlineError::Eof)
            } else {
                let line = self.lines[self.pos].clone();
                self.pos += 1;
                Ok(line)
            }
        }

        fn load_history<P: AsRef<Path> + ?Sized>(&mut self, _path: &P) -> rustyline::Result<()> {
            Ok(())
        }

        fn save_history<P: AsRef<Path> + ?Sized>(&mut self, path: &P) -> rustyline::Result<()> {
            std::fs::write(path, self.history.join("\n"))?;
            Ok(())
        }

        fn add_history_entry<S: AsRef<str> + Into<String>>(&mut self, line: S) -> rustyline::Result<bool> {
            self.history.push(line.as_ref().to_string());
            Ok(true)
        }
    }

    #[test]
    fn test_run_client_select() -> Result<(), anyhow::Error> {
        let work_dir = tempfile::tempdir()?;
        let db_url = format!("jdbc:simpledb:{}", work_dir.path().join("db").to_string_lossy());
        let commands = vec![
            db_url,
            "create table T(A I32)".to_string(),
            "insert into T(A) values (1)".to_string(),
            "select A from T".to_string(),
            "select A from T".to_string(),
            "exit".to_string(),
        ];
        let mut editor = ScriptedEditor::new(commands);
        let current = std::env::current_dir()?;
        std::env::set_current_dir(work_dir.path())?;
        let mut output = Vec::new();
        run_client(
            Driver::Embedded(EmbeddedDriver::new()),
            &mut editor,
            &mut output,
        )?;
        std::env::set_current_dir(current)?;

        use colored::Colorize;
        let output_str = String::from_utf8(output).unwrap();
        let expected = format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
            "0 records processed".magenta(),
            "1 records processed".magenta(),
            format!("{:>12}", "A").bold().cyan(),
            "-".repeat(12).bright_blue(),
            format!("{:>12}", 1).yellow(),
            format!("{:>12}", "A").bold().cyan(),
            "-".repeat(12).bright_blue(),
            format!("{:>12}", 1).yellow(),
        );
        assert_eq!(output_str, expected);

        let history_file = work_dir.path().join(".simpledb_history");
        let history_content = fs::read_to_string(history_file)?;
        assert!(history_content.contains("select A from T"));
        Ok(())
    }
}
