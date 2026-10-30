package sample

import "fmt"

type Greeter interface {
	Greet(name string) string
}

type Book struct {
	ID   string
	Name string
}

func (i *Book) Greet(name string) string {
	return fmt.Sprintf("Hello %s", name)
}

func FetchBooks() []Book {
	return load()
}

const APIBase = "/api/v1/books"
