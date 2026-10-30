import { Book, BookRoute } from "./a/b";
import type { Greeter } from "../core/greeter";

export interface Store {
  load(): Promise<Book[]>;
}

export class BookStore implements Store, Greeter {
  private books: Book[] = [];

  async load(): Promise<Book[]> {
    const route = new BookRoute("1");
    return fetchBooks(route);
  }
}

export function fetchBooks(route: BookRoute): Promise<Book[]> {
  return Promise.resolve([]);
}

export const greet = (name: string): string => `Hello ${name}`;

const API_BASE = "/api/v1/books";
