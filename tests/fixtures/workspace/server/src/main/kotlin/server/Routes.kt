package server

@Resource("/api/v1/books/{id}")
class BookRoute(val id: String)

fun Route.books() {
    get<BookRoute> { route -> call.respond(load(route.id)) }
}
