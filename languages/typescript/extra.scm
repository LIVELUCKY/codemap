;; TypeScript and TSX definitions and references for codemap (grammar: tree-sitter-typescript).
;; Based on the grammar's queries/tags.scm (MIT), extended with supertypes, imports and constants.

;; headers
(interface_declaration name: (type_identifier) @name) @definition.interface
(class_declaration name: (type_identifier) @name) @definition.class
(abstract_class_declaration name: (type_identifier) @name) @definition.class
(enum_declaration name: (identifier) @name) @definition.enum
(type_alias_declaration name: (type_identifier) @name) @definition.typealias
(module name: (identifier) @name) @definition.module
(function_declaration name: (identifier) @name) @definition.function
(function_signature name: (identifier) @name) @definition.function
(method_definition name: (property_identifier) @name) @definition.method
(method_signature name: (property_identifier) @name) @definition.method
(abstract_method_signature name: (property_identifier) @name) @definition.method
(public_field_definition name: (property_identifier) @name) @definition.property

;; const f = () => ... and const f = function ...
(lexical_declaration
  (variable_declarator name: (identifier) @name value: [(arrow_function) (function_expression)]) @definition.function)

;; top-level constants with a literal value
(program
  (lexical_declaration "const"
    (variable_declarator name: (identifier) @name value: [(string) (number) (template_string) (true) (false) (null) (object) (array)]) @definition.constant))
(program
  (export_statement
    (lexical_declaration "const"
      (variable_declarator name: (identifier) @name value: [(string) (number) (template_string) (true) (false) (null) (object) (array)]) @definition.constant)))

;; supertypes
(class_declaration name: (type_identifier) @name (class_heritage (implements_clause (type_identifier) @supertype))) @definition.any
(class_declaration name: (type_identifier) @name (class_heritage (extends_clause value: (identifier) @supertype))) @definition.any
(abstract_class_declaration name: (type_identifier) @name (class_heritage (implements_clause (type_identifier) @supertype))) @definition.any
(abstract_class_declaration name: (type_identifier) @name (class_heritage (extends_clause value: (identifier) @supertype))) @definition.any
(interface_declaration name: (type_identifier) @name (extends_type_clause type: (type_identifier) @supertype)) @definition.any

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
