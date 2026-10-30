use std::fmt;

pub trait Greeter {
    fn greet(&self, name: &str) -> String;
}

pub struct Book {
    pub id: String,
}

impl Greeter for Book {
    fn greet(&self, name: &str) -> String {
        format!("Hello {name}")
    }
}

pub enum State {
    Idle,
    Loaded(Vec<Book>),
}

pub fn fetch_books() -> Vec<Book> {
    load_all()
}

pub const API_BASE: &str = "/api/v1/books";
