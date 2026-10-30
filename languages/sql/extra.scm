;; SQL for codemap (grammar: tree-sitter-sequel). Tables, views, indexes and functions are
;; definitions; a table read from and a function called are references.

(create_table (object_reference name: (identifier) @name)) @definition.table
(create_view (object_reference name: (identifier) @name)) @definition.view
(create_index column: (identifier) @name) @definition.index
(create_function (object_reference name: (identifier) @name)) @definition.function
(relation (object_reference name: (identifier) @name)) @reference.table
(invocation (object_reference name: (identifier) @name)) @reference.call
