module Sample where

data Book = Book { bookId :: String }

newtype Wrapper = Wrapper Book

class Greeter a where
  greet :: a -> String

instance Greeter Book where
  greet (Book i) = "Hello " ++ i

fetchBooks :: IO [Book]
fetchBooks = loadAll

apiBase :: String
apiBase = "/api/v1/books"
