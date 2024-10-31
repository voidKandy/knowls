use std::str::FromStr;

use lsp_types::Uri;

pub fn test_doc_1() -> (Uri, String) {
    let test_doc_1_uri = Uri::from_str("test_doc_1.rs").unwrap();
    let test_doc_1 = r#"use std::io::{self, Read};
// Comment without any command

// @_hey
fn main() {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read io");
}

// +_
struct ToBePushed;
    "#
    .to_string();
    (test_doc_1_uri, test_doc_1)
}

pub fn test_doc_2() -> (Uri, String) {
    let test_doc_2_uri = Uri::from_str("test_doc_2.rs").unwrap();
    let test_doc_2 = r#"
use std::fs;

// +_
fn read_file(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|_| "File not found".to_string())
}

fn main() {
    let content = read_file("example.txt");
    println!("{}", content);
}
    "#
    .to_string();
    (test_doc_2_uri, test_doc_2)
}

pub fn test_doc_3() -> (Uri, String) {
    let test_doc_3_uri = Uri::from_str("test_doc_3.rs").unwrap();
    let test_doc_3 = r#"
fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

// @^Whats up 
fn main() {
    let result = fibonacci(10);
    println!("Fibonacci of 10: {}", result);
}
    "#
    .to_string();
    (test_doc_3_uri, test_doc_3)
}

pub fn test_doc_4() -> (Uri, String) {
    let test_doc_4_uri = Uri::from_str("test_doc_4.rs").unwrap();
    let test_doc_4 = r#"
fn factorial(n: u32) -> u32 {
    (1..=n).product()
}

fn main() {
    let result = factorial(5);
    println!("Factorial of 5: {}", result);
}
    "#
    .to_string();
    (test_doc_4_uri, test_doc_4)
}

pub fn test_doc_5() -> (Uri, String) {
    let test_doc_5_uri = Uri::from_str("test_doc_5.rs").unwrap();
    let test_doc_5 = r#"
use std::collections::HashMap;

// +_
fn count_occurrences(items: Vec<&str>) -> HashMap<&str, usize> {
    let mut occurrences = HashMap::new();
    for item in items {
        *occurrences.entry(item).or_insert(0) += 1;
    }
    occurrences
}
// regular comment
fn main() {
    let items = vec!["apple", "banana", "apple", "orange", "banana", "banana"];
    let counts = count_occurrences(items);
    println!("{:?}", counts);
}
    "#
    .to_string();
    (test_doc_5_uri, test_doc_5)
}
