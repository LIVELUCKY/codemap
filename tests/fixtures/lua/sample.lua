local M = {}

API_BASE = "/api/v1/books"

function M.greet(name)
  return "Hello " .. name
end

local function load()
  return fetch_books()
end

function M.load_all()
  return load()
end

return M
