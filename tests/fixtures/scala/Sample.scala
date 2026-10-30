package sample

trait Greeter {
  def greet(name: String): String
}

abstract class Base

class BookStore extends Base with Greeter {
  def greet(name: String): String = s"Hello $name"

  def load(): List[String] = fetchBooks()
}

object BookStore {
  val ApiBase = "/api/v1/books"
}

case class Book(id: String)
