package app

class FakeBookPort : BookPort {
    override suspend fun load(id: String): Book = Book(id, "fake")
}
