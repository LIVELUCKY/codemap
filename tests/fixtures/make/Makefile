CC := gcc

build: main.o
	$(CC) -o app main.o

.PHONY: test
test: build
	./app --test

define greet
	echo hi
endef
