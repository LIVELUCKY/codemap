package sample;

import java.util.List;

interface Greeter {
    String greet(String name);
}

abstract class Base {}

public class BookStore extends Base implements Greeter, Runnable {
    private final List<String> books = List.of();

    public String greet(String name) {
        return "Hello " + name;
    }

    public void run() {
        load();
    }

    static final String API_BASE = "/api/v1/books";
}
