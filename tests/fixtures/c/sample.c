#include <stdio.h>

struct book {
    int id;
};

typedef struct book book_t;

static int load(void) {
    return 0;
}

int fetch_books(book_t *out) {
    return load();
}
