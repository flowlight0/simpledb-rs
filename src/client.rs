use std::io::{stdin, stdout, Write};

use simpledb_rs::{
    driver::{embedded::EmbeddedDriver, Statement},
    record::field::Type,
};

fn do_query(statement: &mut Box<dyn Statement>, command: &str) -> Result<(), anyhow::Error> {
    let mut result_set = statement.execute_query(command)?;
    let metadata = result_set.get_metadata();
    let num_columns = metadata.get_column_count();
    let mut total_width = 0;

    // print header
    for i in 0..num_columns {
        let column_name = metadata.get_column_name(i);
        let mut width = metadata.get_column_display_size(i);
        if i > 0 {
            width += 1;
        }

        total_width += width;
        print!("{:>width$}", column_name, width = width);
    }
    println!();
    for _ in 0..total_width {
        print!("-");
    }
    println!();

    // print records
    while result_set.next()? {
        for i in 0..num_columns {
            let column_name = metadata.get_column_name(i);
            let column_type = metadata.get_column_type(i);
            let width = metadata.get_column_display_size(i);
            // String fmt = "%" + md.getColumnDisplaySize(i);
            if i > 0 {
                print!(" ");
            }

            match column_type {
                Type::I32 => {
                    let val = result_set.get_i32(&column_name)?;
                    print!("{:>width$}", val, width = width);
                }
                Type::String => {
                    let val = result_set.get_string(&column_name)?;
                    print!("{:>width$}", val, width = width);
                }
            }
        }
        println!();
    }
    result_set.close()?;
    Ok(())
}

fn do_update(statement: &mut Box<dyn Statement>, command: &str) -> Result<(), anyhow::Error> {
    let num_records = statement.execute_update(command)?;
    println!("{} records processed", num_records);
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    print!("Connect> ");
    stdout().flush()?;
    let mut db_url = String::new();
    stdin().read_line(&mut db_url).unwrap();

    let driver = if db_url.contains("//") {
        todo!()
    } else {
        EmbeddedDriver::new()
    };

    let (db_name, mut connection) = driver.connect(&db_url)?;
    let mut statement = connection.create_statement()?;

    print!("\nSQL ({})> ", &db_name);
    stdout().flush()?;
    loop {
        // process one line of input
        let mut command = String::new();
        stdin().read_line(&mut command).unwrap();
        if command.starts_with("exit") {
            break;
        }

        let result = if command.starts_with("SELECT") {
            do_query(&mut statement, &command)
        } else {
            do_update(&mut statement, &command)
        };

        if let Err(e) = result {
            eprintln!("Error: {}", e);
            connection.rollback()?;
        }
        print!("\nSQL ({})> ", &db_name);
        stdout().flush()?;
    }
    connection.commit()?;
    connection.close()?;
    Ok(())
}
