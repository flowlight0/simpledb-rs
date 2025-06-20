use simpledb_rs::driver::{
    embedded::EmbeddedDriver, ConnectionControl, DriverControl, StatementControl,
};

fn main() -> Result<(), anyhow::Error> {
    let driver = EmbeddedDriver::new();
    let url = "jdbc:simpledb:studentdb";
    let (_db_name, mut conn) = driver.connect(url)?;
    let mut stmt = conn.create_statement()?;

    // STUDENT table
    stmt.execute_update(
        "create table STUDENT(SId I32, SName VARCHAR(10), MajorId I32, GradYear I32)",
    )?;
    println!("Table STUDENT created.");
    let studvals = [
        "(1, 'joe', 10, 2021)",
        "(2, 'amy', 20, 2020)",
        "(3, 'max', 10, 2022)",
        "(4, 'sue', 20, 2022)",
        "(5, 'bob', 30, 2020)",
        "(6, 'kim', 20, 2020)",
        "(7, 'art', 30, 2021)",
        "(8, 'pat', 20, 2019)",
        "(9, 'lee', 10, 2021)",
    ];
    for val in studvals.iter() {
        stmt.execute_update(&format!(
            "insert into STUDENT(SId, SName, MajorId, GradYear) values {}",
            val
        ))?;
    }
    println!("STUDENT records inserted.");

    // DEPT table
    stmt.execute_update("create table DEPT(DId I32, DName VARCHAR(8))")?;
    println!("Table DEPT created.");
    let deptvals = ["(10, 'compsci')", "(20, 'math')", "(30, 'drama')"];
    for val in deptvals.iter() {
        stmt.execute_update(&format!("insert into DEPT(DId, DName) values {}", val))?;
    }
    println!("DEPT records inserted.");

    // COURSE table
    stmt.execute_update("create table COURSE(CId I32, Title VARCHAR(20), DeptId I32)")?;
    println!("Table COURSE created.");
    let coursevals = [
        "(12, 'db systems', 10)",
        "(22, 'compilers', 10)",
        "(32, 'calculus', 20)",
        "(42, 'algebra', 20)",
        "(52, 'acting', 30)",
        "(62, 'elocution', 30)",
    ];
    for val in coursevals.iter() {
        stmt.execute_update(&format!(
            "insert into COURSE(CId, Title, DeptId) values {}",
            val
        ))?;
    }
    println!("COURSE records inserted.");

    // SECTION table
    stmt.execute_update(
        "create table SECTION(SectId I32, CourseId I32, Prof VARCHAR(8), YearOffered I32)",
    )?;
    println!("Table SECTION created.");
    let sectvals = [
        "(13, 12, 'turing', 2018)",
        "(23, 12, 'turing', 2019)",
        "(33, 32, 'newton', 2019)",
        "(43, 32, 'einstein', 2017)",
        "(53, 62, 'brando', 2018)",
    ];
    for val in sectvals.iter() {
        stmt.execute_update(&format!(
            "insert into SECTION(SectId, CourseId, Prof, YearOffered) values {}",
            val
        ))?;
    }
    println!("SECTION records inserted.");

    // ENROLL table
    stmt.execute_update(
        "create table ENROLL(EId I32, StudentId I32, SectionId I32, Grade VARCHAR(2))",
    )?;
    println!("Table ENROLL created.");
    let enrollvals = [
        "(14, 1, 13, 'A')",
        "(24, 1, 43, 'C')",
        "(34, 2, 43, 'B+')",
        "(44, 4, 33, 'B')",
        "(54, 4, 53, 'A')",
        "(64, 6, 53, 'A')",
    ];
    for val in enrollvals.iter() {
        stmt.execute_update(&format!(
            "insert into ENROLL(EId, StudentId, SectionId, Grade) values {}",
            val
        ))?;
    }
    println!("ENROLL records inserted.");

    conn.close()?;
    Ok(())
}
