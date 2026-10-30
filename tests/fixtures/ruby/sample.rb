require "json"

module Greeter
  def greet(name)
    "Hello #{name}"
  end
end

class Base; end

class BookStore < Base
  include Greeter

  def load
    fetch_books
  end
end

API_BASE = "/api/v1/books"
