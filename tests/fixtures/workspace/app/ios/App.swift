import Shared

struct App {
    func start() {
        print(PlatformKt.platformName())
        let route = BookRoute(id: "1")
        print(route)
    }
}
