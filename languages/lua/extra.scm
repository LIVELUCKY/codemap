;; Lua for codemap (grammar: tree-sitter-lua). Based on the grammar's queries/tags.scm (MIT),
;; extended with top-level assignments.

(function_declaration name: [(identifier) @name (dot_index_expression field: (identifier) @name)]) @definition.function
(function_declaration name: (method_index_expression method: (identifier) @name)) @definition.method
(assignment_statement
  (variable_list . name: [(identifier) @name (dot_index_expression field: (identifier) @name)])
  (expression_list . value: (function_definition))) @definition.function
(table_constructor (field name: (identifier) @name value: (function_definition))) @definition.function
(chunk (assignment_statement (variable_list name: (identifier) @name)) @definition.property)
(chunk (variable_declaration (assignment_statement (variable_list name: (identifier) @name))) @definition.property)

(function_call
  name: [
    (identifier) @name
    (dot_index_expression field: (identifier) @name)
    (method_index_expression method: (identifier) @name)
  ]) @reference.call
(string (string_content) @literal)
