;; C++ definitions and references for codemap (grammar: tree-sitter-cpp).
;; Based on the grammar's queries/tags.scm (MIT), extended with namespaces, base classes, calls and
;; includes.

(struct_specifier name: (type_identifier) @name body: (_)) @definition.struct
(union_specifier name: (type_identifier) @name body: (_)) @definition.struct
(class_specifier name: (type_identifier) @name) @definition.class
(enum_specifier name: (type_identifier) @name) @definition.enum
(namespace_definition name: (namespace_identifier) @name) @definition.module
(function_declarator declarator: (identifier) @name) @definition.function
(function_declarator declarator: (field_identifier) @name) @definition.method
(function_declarator declarator: (qualified_identifier name: (identifier) @name)) @definition.method
(type_definition declarator: (type_identifier) @name) @definition.typealias
(alias_declaration name: (type_identifier) @name) @definition.typealias
(preproc_def name: (identifier) @name) @definition.constant
(preproc_function_def name: (identifier) @name) @definition.macro

(class_specifier name: (type_identifier) @name (base_class_clause (type_identifier) @supertype)) @definition.any
(struct_specifier name: (type_identifier) @name (base_class_clause (type_identifier) @supertype)) @definition.any

(preproc_include path: (_) @name) @reference.import

(call_expression function: (identifier) @name) @reference.call
(call_expression function: (field_expression field: (field_identifier) @name)) @reference.call
(call_expression function: (qualified_identifier name: (identifier) @name)) @reference.call
