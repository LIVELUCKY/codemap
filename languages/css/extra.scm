;; CSS for codemap (grammar: tree-sitter-css). Class and id selectors are definitions, as are custom
;; properties (`--name`).

(class_selector (class_name) @name) @definition.class
(id_selector (id_name) @name) @definition.id
((declaration (property_name) @name) @definition.property
  (#match? @name "^--"))
(import_statement (string_value) @literal)
