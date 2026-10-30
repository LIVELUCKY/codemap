#include <string>

namespace sample {

class Base {
public:
    virtual ~Base() = default;
};

class BookStore : public Base {
public:
    std::string greet(const std::string& name);
};

std::string BookStore::greet(const std::string& name) {
    return load(name);
}

int fetchBooks() {
    return 0;
}

}
