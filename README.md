# simpledb-rs

[![Coverage Status](https://coveralls.io/repos/github/flowlight0/simpledb-rs/badge.svg?branch=main)](https://coveralls.io/github/flowlight0/simpledb-rs?branch=main)

This repository is a Rust implementation of SimpleDB from the book "[Database Design and Implementation](https://link.springer.com/book/10.1007/978-3-030-33836-7)" by Edward Sciore.

## Overview

The main goal of this repoistory is my own learning. The code from Chapter 3 to Chapter 15 of the book was originally implemented in Java, and I have re-implemented it in Rust. I will add the code for the exercises of each chapter when I feel like it.

## Completed excercises

The following issues correspond to finished exercises from the book:

- [Exercise 3.15](https://github.com/flowlight0/simpledb-rs/issues/33): Add diagnostic routines to FileManager
- [Exercise 6.10](https://github.com/flowlight0/simpledb-rs/issues/66): Add previous and after_last methods to TableScan and RecordPage
- [Exercise 6.13](https://github.com/flowlight0/simpledb-rs/issues/74): Revise the record manager to handle null field values
- [Exercise 8.11](https://github.com/flowlight0/simpledb-rs/issues/77): Handle null in the query processor
- [Exercise 8.13](https://github.com/flowlight0/simpledb-rs/issues/69): Implement previous and after_last for more scans
- [Exercise 9.15](https://github.com/flowlight0/simpledb-rs/issues/79): Introduce "*" character in the select clause
- [Exercise 9.18](https://github.com/flowlight0/simpledb-rs/issues/83): More null handling in query processing
- [Exercise 11.4](https://github.com/flowlight0/simpledb-rs/issues/63): Implement methods 'beforeFirst' and 'absolute(int n)' for ResultSet
- [Exercise 11.5](https://github.com/flowlight0/simpledb-rs/issues/72): Modify ResultSet to support previous and after_last
- [Exercise 11.6](https://github.com/flowlight0/simpledb-rs/issues/85): Implement wasNull method for ResultSet

## Interactive Client

The `client` binary now uses `rustyline` for input. This allows convenient line editing and command history. Previous statements are stored in a `.simpledb_history` file in the current directory and loaded automatically the next time the client runs.

## Useful links

- https://github.com/mnogu/simpledb-rs: Better implementation of SimpleDB in Rust
- https://zenn.dev/hmarui66/scraps/850df4edc50c58: Well organized Japanese summary of each chapter of the book
