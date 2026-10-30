const std = @import("std");

pub const api_base = "/api/v1/books";

pub const Book = struct {
    id: []const u8,
};

pub fn greet(name: []const u8) []const u8 {
    return load(name);
}

fn load(name: []const u8) []const u8 {
    return name;
}
