;; C definitions and references for codemap (grammar: tree-sitter-c).
;; Based on the grammar's queries/tags.scm (MIT), extended with calls and includes.

(struct_specifier name: (type_identifier) @name body: (_)) @definition.struct
(union_specifier name: (type_identifier) @name body: (_)) @definition.struct
(enum_specifier name: (type_identifier) @name) @definition.enum
(function_declarator declarator: (identifier) @name) @definition.function
(type_definition declarator: (type_identifier) @name) @definition.typealias
(preproc_def name: (identifier) @name) @definition.constant
(preproc_function_def name: (identifier) @name) @definition.macro

(preproc_include path: (_) @name) @reference.import

(call_expression function: (identifier) @name) @reference.call
(call_expression function: (field_expression field: (field_identifier) @name)) @reference.call
