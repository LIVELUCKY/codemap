import 'package:sample/core.dart';

abstract class Greeter {
  String greet(String name);
}

class Base {}

class BookStore extends Base implements Greeter {
  @override
  String greet(String name) => 'Hello $name';

  List<String> load() {
    return fetchBooks();
  }
}

List<String> fetchBooks() => [];

const apiBase = '/api/v1/books';
