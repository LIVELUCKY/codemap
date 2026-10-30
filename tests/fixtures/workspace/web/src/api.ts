export const BOOK_URL = "/api/v1/books/{id}";

export function fetchBook(id: string): Promise<Response> {
  return fetch(BOOK_URL.replace("{id}", id));
}
