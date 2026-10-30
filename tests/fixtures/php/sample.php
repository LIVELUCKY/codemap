<?php

namespace Sample;

interface Greeter {
    public function greet(string $name): string;
}

class Base {}

class BookStore extends Base implements Greeter {
    public function greet(string $name): string {
        return load($name);
    }
}

function fetchBooks(): array {
    return [];
}

const API_BASE = "/api/v1/books";
