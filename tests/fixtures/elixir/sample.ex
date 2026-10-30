defmodule Sample.BookStore do
  @api_base "/api/v1/books"

  def greet(name) do
    "Hello #{name}"
  end

  defp load do
    fetch_books()
  end

  def load_all, do: load()
end
