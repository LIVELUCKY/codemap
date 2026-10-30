type book = { id : string }

module Store = struct
  let load () = fetch_books ()
end

let greet name = "Hello " ^ name

let api_base = "/api/v1/books"
