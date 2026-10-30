package server

class RoutesTest {
    fun `book route round trips`() {
        val route = BookRoute("1")
        check(route.id == "1")
    }
}
