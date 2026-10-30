import Foundation
import Shared

protocol Greeter {
    func greet(name: String) -> String
}

struct Book: Codable, Equatable {
    let id: String
    let name: String
}

class BookStore: Greeter {
    var books: [Book] = []

    func greet(name: String) -> String {
        return "Hello \(name)"
    }

    func load() {
        KoinHelperKt.doInit()
        let route = BookRoute(id: "1")
        print(route)
    }
}

extension BookStore {
    func count() -> Int { books.count }
}

enum State {
    case idle
    case loaded([Book])
}

func platformName() -> String { "ios" }
