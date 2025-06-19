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
            total_width += 3;
        }
        total_width += width;
        widths.push(width);
        names.push(column_name.clone());
        types_vec.push(column_type);

        let header = format!("{:>width$}", column_name, width = width);
        if i > 0 {
            write!(writer, " | ")?;
        }
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
                        segs.push(format!("{:<width$}", part, width = width));
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
                    write!(writer, " | ")?;
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

fn do_show_tables<W: Write>(
    statement: &mut Statement,
    writer: &mut W,
) -> Result<(), anyhow::Error> {
    // Get all table names from tblcat
    let mut result_set = statement.execute_query("select tblname from tblcat")?;
    let mut table_names = Vec::new();
    while result_set.next()? {
        if let Some(name) = result_set.get_string("tblname")? {
            table_names.push(name);
        }
    }
    result_set.close()?;

    let mut rows = Vec::new();
    for table_name in table_names {
        let query = format!(
            "select fldname, type, length from fldcat where tblname = '{}'",
            table_name
        );
        let mut rs = statement.execute_query(&query)?;
        let mut parts = Vec::new();
        while rs.next()? {
            let fname = rs.get_string("fldname")?.unwrap_or_default();
            let tcode = rs.get_i32("type")?.unwrap_or(0);
            let length = rs.get_i32("length")?.unwrap_or(0);
            let type_str = match Type::from_code(tcode) {
                Ok(Type::I32) => "I32".to_string(),
                Ok(Type::String) => format!("VARCHAR({})", length),
                Err(_) => format!("{}", tcode),
            };
            parts.push(format!("{} {}", fname, type_str));
        }
        rs.close()?;
        rows.push((table_name, parts.join(", ")));
    }

    let name_header = "name".to_string();
    let schema_header = "schema".to_string();
    let mut name_width = name_header.len();
    let mut schema_width = schema_header.len();
    for (name, schema) in &rows {
        name_width = std::cmp::max(name_width, name.len());
        schema_width = std::cmp::max(schema_width, schema.len());
    }
    let total_width = name_width + 3 + schema_width;

    let header_name = format!("{:>width$}", name_header, width = name_width);
    let header_schema = format!("{:>width$}", schema_header, width = schema_width);
    write!(writer, "{}", header_name.bold().cyan())?;
    write!(writer, " | ")?;
    write!(writer, "{}", header_schema.bold().cyan())?;
    writeln!(writer)?;
    writeln!(writer, "{}", "-".repeat(total_width).bright_blue())?;

    for (name, schema) in rows {
        let name_cells = {
            let chars: Vec<char> = name.chars().collect();
            let mut segs = Vec::new();
            for chunk in chars.chunks(name_width) {
                let part: String = chunk.iter().collect();
                segs.push(format!("{:<width$}", part, width = name_width));
            }
            if segs.is_empty() {
                segs.push(" ".repeat(name_width));
            }
            segs
        };
        let schema_cells = {
            let chars: Vec<char> = schema.chars().collect();
            let mut segs = Vec::new();
            for chunk in chars.chunks(schema_width) {
                let part: String = chunk.iter().collect();
                segs.push(format!("{:<width$}", part, width = schema_width));
            }
            if segs.is_empty() {
                segs.push(" ".repeat(schema_width));
            }
            segs
        };
        let max_lines = std::cmp::max(name_cells.len(), schema_cells.len());
        for i in 0..max_lines {
            let name_seg = name_cells
                .get(i)
                .cloned()
                .unwrap_or_else(|| " ".repeat(name_width));
            let schema_seg = schema_cells
                .get(i)
                .cloned()
                .unwrap_or_else(|| " ".repeat(schema_width));
            write!(writer, "{} | {}", name_seg.green(), schema_seg.green())?;
            writeln!(writer)?;
        }
    }
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
    db_url: Option<&str>,
) -> Result<(), anyhow::Error> {
    let history_path = PathBuf::from(".simpledb_history");
    let _ = editor.load_history(&history_path);

    let db_url = match db_url {
        Some(url) => url.to_string(),
        None => loop {
            match editor.readline("Connect> ") {
                Ok(line) => break line,
                Err(ReadlineError::Interrupted) => continue,
                Err(ReadlineError::Eof) => return Ok(()),
                Err(e) => return Err(e.into()),
            }
        },
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

        let upper = trimmed.to_ascii_uppercase();
        let result = if upper == "SHOW TABLES" {
            do_show_tables(&mut statement, writer)
        } else if upper.starts_with("SELECT") {
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
    let db_url = std::env::args().nth(1);
    run_client(
        Driver::Embedded(EmbeddedDriver::new()),
        &mut editor,
        &mut stdout(),
        db_url.as_deref(),
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
            Some(&db_url),
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
    fn test_run_client_select_two_columns() -> Result<(), anyhow::Error> {
        let work_dir = tempfile::tempdir()?;
        let db_url = format!(
            "jdbc:simpledb:{}",
            work_dir.path().join("db").to_string_lossy()
        );
        let commands = vec![
            "create table T(A I32, B I32)".to_string(),
            "insert into T(A, B) values (1, 2)".to_string(),
            "select A, B from T".to_string(),
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
            Some(&db_url),
        )?;
        std::env::set_current_dir(current)?;

        use colored::Colorize;
        let output_str = String::from_utf8(output).unwrap();
        let expected = format!(
            "{}\n{}\n{} | {}\n{}\n{} | {}\n",
            "0 records processed".magenta(),
            "1 records processed".magenta(),
            format!("{:>12}", "A").bold().cyan(),
            format!("{:>12}", "B").bold().cyan(),
            "-".repeat(27).bright_blue(),
            format!("{:>12}", 1).yellow(),
            format!("{:>12}", 2).yellow(),
        );
        assert_eq!(output_str, expected);
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
            Some(&db_url),
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

    #[test]
    #[serial_test::serial]
    fn test_run_client_show_tables() -> Result<(), anyhow::Error> {
        let work_dir = tempfile::tempdir()?;
        let db_url = format!(
            "jdbc:simpledb:{}",
            work_dir.path().join("db").to_string_lossy()
        );
        let commands = vec![
            "create table T1(A I32, B VARCHAR(10))".to_string(),
            "create table T2(C I32)".to_string(),
            "show tables".to_string(),
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
            Some(&db_url),
        )?;
        std::env::set_current_dir(current)?;

        use colored::Colorize;
        let output_str = String::from_utf8(output).unwrap();
        // Build expected output using same formatting as do_show_tables
        let mut rows = vec![
            ("tblcat", "slotsize I32, tblname VARCHAR(50)"),
            (
                "fldcat",
                "type I32, length I32, offset I32, tblname VARCHAR(50), fldname VARCHAR(50)",
            ),
            (
                "idxcat",
                "index_name VARCHAR(255), table_name VARCHAR(255), field_name VARCHAR(255)",
            ),
            ("T1", "A I32, B VARCHAR(10)"),
            ("T2", "C I32"),
        ];
        let name_header = "name";
        let schema_header = "schema";
        let mut name_width = name_header.len();
        let mut schema_width = schema_header.len();
        for (n, s) in &rows {
            name_width = std::cmp::max(name_width, n.len());
            schema_width = std::cmp::max(schema_width, s.len());
        }
        let total_width = name_width + 3 + schema_width;

        let mut expected = String::new();
        expected.push_str(&format!("{}\n", "0 records processed".magenta()));
        expected.push_str(&format!("{}\n", "0 records processed".magenta()));
        expected.push_str(&format!(
            "{} | {}\n",
            format!("{:>width$}", name_header, width = name_width)
                .bold()
                .cyan(),
            format!("{:>width$}", schema_header, width = schema_width)
                .bold()
                .cyan()
        ));
        expected.push_str(&format!("{}\n", "-".repeat(total_width).bright_blue()));
        for (name, schema) in rows.drain(..) {
            let name_cells = {
                let chars: Vec<char> = name.chars().collect();
                let mut segs = Vec::new();
                for chunk in chars.chunks(name_width) {
                    let part: String = chunk.iter().collect();
                    segs.push(format!("{:<width$}", part, width = name_width));
                }
                if segs.is_empty() {
                    segs.push(" ".repeat(name_width));
                }
                segs
            };
            let schema_cells = {
                let chars: Vec<char> = schema.chars().collect();
                let mut segs = Vec::new();
                for chunk in chars.chunks(schema_width) {
                    let part: String = chunk.iter().collect();
                    segs.push(format!("{:<width$}", part, width = schema_width));
                }
                if segs.is_empty() {
                    segs.push(" ".repeat(schema_width));
                }
                segs
            };
            let max_lines = std::cmp::max(name_cells.len(), schema_cells.len());
            for i in 0..max_lines {
                let n = name_cells
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| " ".repeat(name_width));
                let s = schema_cells
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| " ".repeat(schema_width));
                expected.push_str(&format!("{} | {}\n", n.green(), s.green()));
            }
        }

        assert_eq!(output_str, expected);
        Ok(())
    }
}
