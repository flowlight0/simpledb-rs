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
    fn add_history_entry<S: AsRef<str> + Into<String>>(
        &mut self,
        line: S,
    ) -> rustyline::Result<bool>;
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

    fn add_history_entry<S: AsRef<str> + Into<String>>(
        &mut self,
        line: S,
    ) -> rustyline::Result<bool> {
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
    let mut widths = Vec::with_capacity(num_columns);
    let mut names = Vec::with_capacity(num_columns);
    let mut types_vec = Vec::with_capacity(num_columns);

    // print header
    for i in 0..num_columns {
        let column_name = metadata.get_column_name(i)?;
        let column_type = metadata.get_column_type(i)?;
        let width = metadata.get_column_display_size(i)?;
        if i > 0 {
            total_width += 1;
        }
        total_width += width;
        widths.push(width);
        names.push(column_name.clone());
        types_vec.push(column_type);

        let header = format!("{:>width$}", column_name, width = width);
        write!(writer, "{}", header.bold().cyan())?;
    }
    writeln!(writer)?;
    let separator = "-".repeat(total_width);
    writeln!(writer, "{}", separator.bright_blue())?;

    // print records
    while result_set.next()? {
        let mut rows: Vec<Vec<String>> = Vec::with_capacity(num_columns);
        let mut max_lines = 1usize;

        for i in 0..num_columns {
            let width = widths[i];
            let column_type = &types_vec[i];
            let name = &names[i];
            let cells = match column_type {
                Type::I32 => {
                    let val_opt = result_set.get_i32(name)?;
                    let val_display = match val_opt {
                        Some(v) => v.to_string(),
                        None => "NULL".to_string(),
                    };
                    vec![format!("{:>width$}", val_display, width = width)]
                }
                Type::String => {
                    let val_opt = result_set.get_string(name)?;
                    let val_display = match val_opt {
                        Some(v) => v,
                        None => "NULL".to_string(),
                    };
                    let chars: Vec<char> = val_display.chars().collect();
                    let mut segs = Vec::new();
                    for chunk in chars.chunks(width) {
                        let part: String = chunk.iter().collect();
                        segs.push(format!("{:>width$}", part, width = width));
                    }
                    if segs.is_empty() {
                        segs.push(" ".repeat(width));
                    }
                    max_lines = std::cmp::max(max_lines, segs.len());
                    segs
                }
            };
            rows.push(cells);
        }

        for line_idx in 0..max_lines {
            for i in 0..num_columns {
                if i > 0 {
                    write!(writer, " ")?;
                }
                let width = widths[i];
                let seg = rows[i]
                    .get(line_idx)
                    .cloned()
                    .unwrap_or_else(|| " ".repeat(width));
                let column_type = &types_vec[i];
                match column_type {
                    Type::I32 => write!(writer, "{}", seg.yellow())?,
                    Type::String => write!(writer, "{}", seg.green())?,
                }
            }
            writeln!(writer)?;
        }
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

fn run_client<W: Write, E: ClientEditor>(
    driver: Driver,
    editor: &mut E,
    writer: &mut W,
) -> Result<(), anyhow::Error> {
    let history_path = PathBuf::from(".simpledb_history");
    let _ = editor.load_history(&history_path);

    let db_url = loop {
        match editor.readline("Connect> ") {
            Ok(line) => break line,
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => return Ok(()),
            Err(e) => return Err(e.into()),
        }
    };

    let (db_name, mut connection) = driver.connect(db_url.trim_end())?;
    let mut statement = connection.create_statement()?;

    loop {
        let prompt = format!("\nSQL ({})> ", db_name);
        let command = match editor.readline(&prompt) {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => break,
            Err(e) => return Err(e.into()),
        };

        let trimmed = command.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("exit") {
            break;
        }

        let _ = editor.add_history_entry(command.as_str());

        let result = if trimmed.to_ascii_uppercase().starts_with("SELECT") {
            do_query(&mut statement, trimmed, writer)
        } else {
            do_update(&mut statement, trimmed, writer)
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
    use simpledb_rs::driver::{embedded::EmbeddedDriver, Driver};
    use std::fs;
    use std::path::Path;

    struct ScriptedEditor {
        lines: Vec<String>,
        pos: usize,
        history: Vec<String>,
    }

    impl ScriptedEditor {
        fn new<I: Into<String>>(lines: Vec<I>) -> Self {
            Self {
                lines: lines.into_iter().map(Into::into).collect(),
                pos: 0,
                history: Vec::new(),
            }
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

        fn add_history_entry<S: AsRef<str> + Into<String>>(
            &mut self,
            line: S,
        ) -> rustyline::Result<bool> {
            self.history.push(line.as_ref().to_string());
            Ok(true)
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_run_client_select() -> Result<(), anyhow::Error> {
        let work_dir = tempfile::tempdir()?;
        let db_url = format!(
            "jdbc:simpledb:{}",
            work_dir.path().join("db").to_string_lossy()
        );
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

    #[test]
    #[serial_test::serial]
    fn test_run_client_ignore_empty_input() -> Result<(), anyhow::Error> {
        let work_dir = tempfile::tempdir()?;
        let db_url = format!(
            "jdbc:simpledb:{}",
            work_dir.path().join("db").to_string_lossy()
        );
        let commands = vec![
            db_url,
            "".to_string(),
            "create table T(A I32)".to_string(),
            "insert into T(A) values (1)".to_string(),
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
            "{}\n{}\n{}\n{}\n{}\n",
            "0 records processed".magenta(),
            "1 records processed".magenta(),
            format!("{:>12}", "A").bold().cyan(),
            "-".repeat(12).bright_blue(),
            format!("{:>12}", 1).yellow(),
        );
        assert_eq!(output_str, expected);

        let history_file = work_dir.path().join(".simpledb_history");
        let history_content = fs::read_to_string(history_file)?;
        assert!(!history_content.contains("\n\n"));
        Ok(())
    }
}
