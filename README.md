# simpledb-rs

[![Coverage Status](https://coveralls.io/repos/github/flowlight0/simpledb-rs/badge.svg?branch=main)](https://coveralls.io/github/flowlight0/simpledb-rs?branch=main)

This repository is a Rust implementation of SimpleDB from the book "[Database Design and Implementation](https://link.springer.com/book/10.1007/978-3-030-33836-7)" by Edward Sciore.

## Overview

The main goal of this repoistory is my own learning. The code from Chapter 3 to Chapter 15 of the book was originally implemented in Java, and I have re-implemented it in Rust.

## Usage

Three binaries are available:

- `client`: A SQL client for interacting with the database.
  - It receives three command line arguments:
    - `db`: The name of the database to connect to.
    - `host`: The host where the database server is running. If it is not specified, the client will connect to local DB with the embedded driver. Otherwise, it will connect to the remote DB server with the specified host and port.
    - `port`: The port on which the database server is listening (default: `50051`). If the `host` is not specified, this argument is ignored.
- `server`: The database server that handles client requests.
- `create_student_db`: Creates a sample database for the student records.

You can run these binaries using `cargo run --bin <binary_name>`. For example, to run the client, use:

```bash
cargo run --bin client studentdb
```

### Example (embedded)

```bash
➜  simpledb-rs git:(main) ✗ cargo run --quiet --bin create_student_db # Create STUDENT DB for DEMO
Table STUDENT created.
STUDENT records inserted.
Table DEPT created.
DEPT records inserted.
Table COURSE created.
COURSE records inserted.
Table SECTION created.
SECTION records inserted.
Table ENROLL created.
ENROLL records inserted.
➜  simpledb-rs git:(main) ✗ cargo run --quiet --bin client studentdb
SQL (studentdb)> show tables
   name |                                                                     schema
------------------------------------------------------------------------------------
tblcat  | slotsize I32, tblname VARCHAR(50)
fldcat  | type I32, length I32, offset I32, tblname VARCHAR(50), fldname VARCHAR(50)
idxcat  | index_name VARCHAR(255), table_name VARCHAR(255), field_name VARCHAR(255)
STUDENT | SId I32, MajorId I32, GradYear I32, SName VARCHAR(10)
DEPT    | DId I32, DName VARCHAR(8)
COURSE  | CId I32, DeptId I32, Title VARCHAR(20)
SECTION | SectId I32, CourseId I32, YearOffered I32, Prof VARCHAR(8)
ENROLL  | EId I32, StudentId I32, SectionId I32, Grade VARCHAR(2)

SQL (studentdb)> select * from STUDENT
         SId |      MajorId |     GradYear |      SName
-------------------------------------------------------
           1 |           10 |         2021 | joe
           2 |           20 |         2020 | amy
           3 |           10 |         2022 | max
           4 |           20 |         2022 | sue
           5 |           30 |         2020 | bob
           6 |           20 |         2020 | kim
           7 |           30 |         2021 | art
           8 |           20 |         2019 | pat
           9 |           10 |         2021 | lee

SQL (studentdb)> select SName, DName, Grade, YearOffered from STUDENT, DEPT, ENROLL, SECTION where SId = StudentId and SectId = SectionId and DId = MajorId and YearOffered = 2018
 YearOffered |      SName |    DName | Grade
--------------------------------------------
        2018 | joe        | compsci  | A
        2018 | sue        | math     | A
        2018 | kim        | math     | A

SQL (studentdb)> select * from ENROLL
         EId |    StudentId |    SectionId | Grade
--------------------------------------------------
          14 |            1 |           13 | A
          24 |            1 |           43 | C
          34 |            2 |           43 | B+
          44 |            4 |           33 | B
          54 |            4 |           53 | A
          64 |            6 |           53 | A

SQL (studentdb)> MODIFY ENROLL SET Grade = 'A+' WHERE StudentId = 6
1 records processed

SQL (studentdb)> select SName, DName, Grade, YearOffered from STUDENT, DEPT, ENROLL, SECTION where SId = StudentId and SectId = SectionId and DId = MajorId and YearOffered = 2018
 YearOffered |      SName |    DName | Grade
--------------------------------------------
        2018 | joe        | compsci  | A
        2018 | sue        | math     | A
        2018 | kim        | math     | A+

SQL (studentdb)> DELETE FROM STUDENT WHERE SName = 'joe'
1 records processed

SQL (studentdb)> select * from STUDENT
         SId |      MajorId |     GradYear |      SName
-------------------------------------------------------
           2 |           20 |         2020 | amy
           3 |           10 |         2022 | max
           4 |           20 |         2022 | sue
           5 |           30 |         2020 | bob
           6 |           20 |         2020 | kim
           7 |           30 |         2021 | art
           8 |           20 |         2019 | pat
           9 |           10 |         2021 | lee

SQL (studentdb)> exit

```

## Completed excercises

The following issues correspond to finished exercises from the book:

- [Exercise 3.15](https://github.com/flowlight0/simpledb-rs/issues/33): Add diagnostic routines to FileManager
- [Exercise 6.10](https://github.com/flowlight0/simpledb-rs/issues/66): Add previous and after_last methods to TableScan and RecordPage
- [Exercise 6.13](https://github.com/flowlight0/simpledb-rs/issues/74): Revise the record manager to handle null field values
- [Exercise 7.7](https://github.com/flowlight0/simpledb-rs/issues/75): Implement 'SHOW TABLES' command in the client
- [Exercise 8.8](https://github.com/flowlight0/simpledb-rs/issues/116): Expressions handle arithmetic operators on integers
- [Exercise 8.11](https://github.com/flowlight0/simpledb-rs/issues/77): Handle null in the query processor
- [Exercise 8.13](https://github.com/flowlight0/simpledb-rs/issues/69): Implement previous and after_last for more scans
- [Exercise 8.15](https://github.com/flowlight0/simpledb-rs/issues/115): Implement ExtendScan
- [Exercise 9.12](https://github.com/flowlight0/simpledb-rs/issues/114): Allow keyword "AS" in queries
- [Exercise 9.15](https://github.com/flowlight0/simpledb-rs/issues/79): Introduce "\*" character in the select clause
- [Exercise 9.18](https://github.com/flowlight0/simpledb-rs/issues/83): More null handling in query processing
- [Exercise 10.14](https://github.com/flowlight0/simpledb-rs/issues/122): Handle `AS` keyword in queries
- [Exercise 11.4](https://github.com/flowlight0/simpledb-rs/issues/63): Implement methods 'beforeFirst' and 'absolute(int n)' for ResultSet
- [Exercise 11.5](https://github.com/flowlight0/simpledb-rs/issues/72): Modify ResultSet to support previous and after_last
- [Exercise 11.6](https://github.com/flowlight0/simpledb-rs/issues/85): Implement wasNull method for ResultSet
- [Exercise 13.15](https://github.com/flowlight0/simpledb-rs/issues/110): Support "ORDER BY" clause
- [Exercise 13.16](https://github.com/flowlight0/simpledb-rs/issues/112): Implement more aggregation functions
- [Exercise 13.7](https://github.com/flowlight0/simpledb-rs/issues/127): Support "GROUP BY" clause

## Interactive Client

The `client` binary now uses `rustyline` for input. This allows convenient line editing and command history. Previous statements are stored in a `.simpledb_history` file in the current directory and loaded automatically the next time the client runs.

## Useful links

- https://github.com/mnogu/simpledb-rs: Better implementation of SimpleDB in Rust
- https://zenn.dev/hmarui66/scraps/850df4edc50c58: Well organized Japanese summary of each chapter of the book
