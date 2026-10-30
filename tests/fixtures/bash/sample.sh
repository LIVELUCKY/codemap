#!/usr/bin/env bash

API_BASE="/api/v1/books"

fetch_books() {
    curl -s "$API_BASE"
}

function greet {
    echo "Hello $1"
}

fetch_books
greet "world"
