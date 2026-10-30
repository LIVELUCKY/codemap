;; OCaml for codemap (grammar: tree-sitter-ocaml). Based on the grammar's queries/tags.scm (MIT),
;; without the doc-comment predicates.

(module_definition (module_binding (module_name) @name)) @definition.module
(module_type_definition (module_type_name) @name) @definition.interface
(class_definition (class_binding (class_name) @name)) @definition.class
(method_definition (method_name) @name) @definition.method
(type_definition (type_binding name: [(type_constructor) @name (type_constructor_path (type_constructor) @name)])) @definition.typealias
(constructor_declaration (constructor_name) @name) @definition.constant
(field_declaration (field_name) @name) @definition.property
;; `let f x = ...` keeps the name as a plain child next to its parameters; `let x = ...` uses the pattern field
(value_definition (let_binding (value_name) @name (parameter) @_p)) @definition.function
(value_definition (let_binding pattern: (value_name) @name body: [(fun_expression) (function_expression)] @_f)) @definition.function
(value_definition (let_binding pattern: (value_name) @name)) @definition.constant

(application_expression function: (value_path (value_name) @name)) @reference.call
(method_invocation (method_name) @name) @reference.call
(open_module (module_path (module_name) @name)) @reference.import
(string (string_content) @literal)
