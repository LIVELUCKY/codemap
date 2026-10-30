package app

interface BookPort {
    suspend fun load(id: String): Book
}

class BookRepositoryImpl(private val api: Api) : BookPort {
    override suspend fun load(id: String): Book = api.get(BookRoute(id))
}

@Resource("/api/v1/books/{id}")
class BookRoute(val id: String)

data class Book(val id: String, val name: String)
