;; JavaScript definitions and references for codemap (grammar: tree-sitter-javascript).
;; Based on the grammar's queries/tags.scm (MIT), without the doc-comment predicates, extended with
;; supertypes and imports.

;; headers
(class_declaration name: (identifier) @name) @definition.class
(class name: (identifier) @name) @definition.class
(method_definition name: (property_identifier) @name) @definition.method
(function_declaration name: (identifier) @name) @definition.function
(generator_function_declaration name: (identifier) @name) @definition.function
(lexical_declaration
  (variable_declarator name: (identifier) @name value: [(arrow_function) (function_expression)]) @definition.function)
(variable_declaration
  (variable_declarator name: (identifier) @name value: [(arrow_function) (function_expression)]) @definition.function)
(assignment_expression
  left: [(identifier) @name (member_expression property: (property_identifier) @name)]
  right: [(arrow_function) (function_expression)]) @definition.function
(pair key: (property_identifier) @name value: [(arrow_function) (function_expression)]) @definition.function

;; top-level constants with a literal value
(program
  (lexical_declaration "const"
    (variable_declarator name: (identifier) @name value: [(string) (number) (template_string) (true) (false) (null) (object) (array)]) @definition.constant))
(program
  (export_statement
    (lexical_declaration "const"
      (variable_declarator name: (identifier) @name value: [(string) (number) (template_string) (true) (false) (null) (object) (array)]) @definition.constant)))

;; supertypes
(class_declaration name: (identifier) @name (class_heritage (identifier) @supertype)) @definition.any

;; imports: the imported name, with the module path as the flag
(import_statement
  (import_clause (named_imports (import_specifier name: (identifier) @name)))
  source: (string (string_fragment) @path)) @reference.import
(import_statement
  (import_clause (identifier) @name)
  source: (string (string_fragment) @path)) @reference.import

;; references
(call_expression function: (identifier) @name) @reference.call
(call_expression function: (member_expression property: (property_identifier) @name)) @reference.call
(new_expression constructor: (identifier) @name) @reference.new
