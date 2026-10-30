CREATE TABLE books (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE INDEX books_name ON books (name);

CREATE VIEW cheap_books AS SELECT id, name FROM books WHERE id < 10;

CREATE FUNCTION book_count() RETURNS INTEGER AS $$ SELECT count(*) FROM books $$ LANGUAGE sql;
