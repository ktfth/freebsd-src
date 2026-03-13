/*-
 * SPDX-License-Identifier: BSD-3-Clause
 *
 * Rust port of FreeBSD's wc(1) utility
 * Original: usr.bin/wc/wc.c
 * Copyright (c) 1980, 1987, 1991, 1993
 *     The Regents of the University of California. All rights reserved.
 *
 * Counts lines, words, characters, and longest line in files (or stdin).
 */

use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::process;

#[derive(Default, Clone)]
struct Counts {
    lines: u64,
    words: u64,
    chars: u64,
    longest_line: u64,
}

#[derive(Default)]
struct Options {
    do_lines: bool,
    do_words: bool,
    do_chars: bool,
    do_longest: bool,
}

impl Options {
    /// If no flag was given, default to -l -w -c
    fn apply_defaults(&mut self) {
        if !self.do_lines && !self.do_words && !self.do_chars && !self.do_longest {
            self.do_lines = true;
            self.do_words = true;
            self.do_chars = true;
        }
    }
}

/// Count lines, words, bytes, and longest line for a single reader.
fn count<R: Read>(reader: R) -> io::Result<Counts> {
    let mut counts = Counts::default();
    let mut in_word = false;
    let mut current_line_len: u64 = 0;

    // Read line-by-line for efficient processing (mirrors buf[MAXBSIZE] in C).
    let mut buf_reader = BufReader::with_capacity(64 * 1024, reader);
    let mut line = Vec::new();

    loop {
        line.clear();
        let bytes_read = buf_reader.read_until(b'\n', &mut line)?;
        if bytes_read == 0 {
            break;
        }

        let has_newline = line.last() == Some(&b'\n');

        // Character (byte) count
        counts.chars += bytes_read as u64;

        // Line count and longest-line tracking
        if has_newline {
            counts.lines += 1;
            // Line content length = previous partial bytes + this chunk minus the newline
            let line_len = current_line_len + bytes_read as u64 - 1;
            if line_len > counts.longest_line {
                counts.longest_line = line_len;
            }
            current_line_len = 0;
        } else {
            current_line_len += bytes_read as u64;
        }

        // Word count: transition from whitespace -> non-whitespace
        for &byte in &line {
            if byte.is_ascii_whitespace() {
                if in_word {
                    counts.words += 1;
                    in_word = false;
                }
            } else {
                in_word = true;
            }
        }
    }

    // Handle file that doesn't end with a newline
    if in_word {
        counts.words += 1;
    }
    if current_line_len > counts.longest_line {
        counts.longest_line = current_line_len;
    }

    Ok(counts)
}

/// Print the selected counts, right-aligned in 7-wide columns (same as BSD wc).
fn print_counts(counts: &Counts, opts: &Options, filename: Option<&str>) {
    if opts.do_lines {
        print!("{:>7}", counts.lines);
    }
    if opts.do_words {
        print!("{:>7}", counts.words);
    }
    if opts.do_chars {
        print!("{:>7}", counts.chars);
    }
    if opts.do_longest {
        print!("{:>7}", counts.longest_line);
    }
    if let Some(name) = filename {
        println!(" {}", name);
    } else {
        println!();
    }
}

fn usage() -> ! {
    eprintln!("usage: wc [-c] [-Llw] [file ...]");
    process::exit(1);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut opts = Options::default();
    let mut file_args: Vec<String> = Vec::new();

    // Argument parsing (mirrors getopt with flags -l -w -c -L)
    let mut parsing_flags = true;
    for arg in args.iter().skip(1) {
        if parsing_flags && arg.starts_with('-') && arg.len() > 1 {
            for ch in arg.chars().skip(1) {
                match ch {
                    'l' => opts.do_lines = true,
                    'w' => opts.do_words = true,
                    'c' => opts.do_chars = true,
                    'L' => opts.do_longest = true,
                    '-' => {
                        parsing_flags = false;
                        break;
                    }
                    _ => usage(),
                }
            }
        } else {
            parsing_flags = false;
            file_args.push(arg.clone());
        }
    }

    opts.apply_defaults();

    let mut totals = Counts::default();
    let mut errors = false;

    if file_args.is_empty() {
        // Read from stdin
        match count(io::stdin().lock()) {
            Ok(c) => {
                print_counts(&c, &opts, None);
                add_totals(&mut totals, &c);
            }
            Err(e) => {
                eprintln!("wc: stdin: {}", e);
                errors = true;
            }
        }
    } else {
        for filename in &file_args {
            match File::open(filename) {
                Ok(f) => match count(f) {
                    Ok(c) => {
                        print_counts(&c, &opts, Some(filename));
                        add_totals(&mut totals, &c);
                    }
                    Err(e) => {
                        eprintln!("wc: {}: {}", filename, e);
                        errors = true;
                    }
                },
                Err(e) => {
                    eprintln!("wc: {}: {}", filename, e);
                    errors = true;
                }
            }
        }

        // Print totals when more than one file
        if file_args.len() > 1 {
            print_counts(&totals, &opts, Some("total"));
        }
    }

    process::exit(if errors { 1 } else { 0 });
}

fn add_totals(totals: &mut Counts, c: &Counts) {
    totals.lines += c.lines;
    totals.words += c.words;
    totals.chars += c.chars;
    if c.longest_line > totals.longest_line {
        totals.longest_line = c.longest_line;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count_str(s: &str) -> Counts {
        count(s.as_bytes()).unwrap()
    }

    #[test]
    fn test_empty() {
        let c = count_str("");
        assert_eq!(c.lines, 0);
        assert_eq!(c.words, 0);
        assert_eq!(c.chars, 0);
    }

    #[test]
    fn test_single_line() {
        let c = count_str("hello world\n");
        assert_eq!(c.lines, 1);
        assert_eq!(c.words, 2);
        assert_eq!(c.chars, 12);
    }

    #[test]
    fn test_multiple_lines() {
        let c = count_str("foo bar\nbaz\n");
        assert_eq!(c.lines, 2);
        assert_eq!(c.words, 3);
        assert_eq!(c.chars, 12);
    }

    #[test]
    fn test_no_trailing_newline() {
        let c = count_str("hello world");
        assert_eq!(c.lines, 0);
        assert_eq!(c.words, 2);
        assert_eq!(c.chars, 11);
    }

    #[test]
    fn test_longest_line() {
        let c = count_str("hi\nhello world\nbye\n");
        assert_eq!(c.longest_line, 11); // "hello world"
    }

    #[test]
    fn test_only_whitespace() {
        let c = count_str("   \n  \n");
        assert_eq!(c.lines, 2);
        assert_eq!(c.words, 0);
    }

    #[test]
    fn test_multiple_spaces_between_words() {
        let c = count_str("one   two    three\n");
        assert_eq!(c.words, 3);
    }
}
