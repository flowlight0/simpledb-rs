# simpledb-rs

[![Coverage Status](https://coveralls.io/repos/github/flowlight0/simpledb-rs/badge.svg?branch=main)](https://coveralls.io/github/flowlight0/simpledb-rs?branch=main)

This repository is a Rust implementation of SimpleDB from the book "[Database Design and Implementation](https://link.springer.com/book/10.1007/978-3-030-33836-7)" by Edward Sciore.

## Overview

The main goal of this repoistory is my own learning. The code from Chapter 3 to Chapter 15 of the book was originally implemented in Java, and I have re-implemented it in Rust. I will add the code for the exercises of each chapter when I feel like it.

## Completed excercises

I'll add the list of completed exercises here.

## Interactive Client

The `client` binary now uses `rustyline` for input. This allows convenient line editing and command history. Previous statements are stored in a `.simpledb_history` file in the current directory and loaded automatically the next time the client runs.

## Useful links

- https://github.com/mnogu/simpledb-rs: Better implementation of SimpleDB in Rust
- https://zenn.dev/hmarui66/scraps/850df4edc50c58: Well organized Japanese summary of each chapter of the book
