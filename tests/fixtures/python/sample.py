from pkg.mod import helper
import os

class Base:
    pass

class BookStore(Base, object):
    def __init__(self):
        self.books = []

    def load(self):
        return helper(self.books)

def fetch_books(store):
    return store.load()

API_BASE = "/api/v1/books"
