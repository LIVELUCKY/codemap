package sample

import kotlinx.serialization.Serializable

interface FooPort {
    suspend fun fetch(): List<Book>
}

abstract class BaseRepository

class FooRepositoryImpl(private val api: Api) : FooPort, BaseRepository() {
    override suspend fun fetch(): List<Book> = api.books()
}

expect fun platformName(): String

sealed class State {
    object Idle : State()
    data class Loaded(val books: List<Book>) : State()
}

data class Book(val id: Id, val name: String)

@Serializable
@Resource("/api/v1/books/{id}")
class BookRoute(val id: String)

class Loader(private val repository: FooPort) {
    suspend fun load(): State {
        val books = repository.fetch()
        return State.Loaded(books)
    }
}

typealias Id = String
