;; Swift definitions and references for codemap (grammar: tree-sitter-swift).
;; Based on the grammar's queries/tags.scm (MIT), extended with struct, enum, extension, inheritance,
;; imports and calls.

;; headers: class, struct, enum, actor and extension share one node; the keyword tells them apart
(class_declaration "class" name: (type_identifier) @name) @definition.class
(class_declaration "struct" name: (type_identifier) @name) @definition.struct
(class_declaration "enum" name: (type_identifier) @name) @definition.enum
(class_declaration "actor" name: (type_identifier) @name) @definition.class
(class_declaration "extension" name: (user_type (type_identifier) @name)) @definition.extension
(protocol_declaration name: (type_identifier) @name) @definition.protocol
(function_declaration name: (simple_identifier) @name) @definition.function
(protocol_function_declaration name: (simple_identifier) @name) @definition.function
(property_declaration (pattern (simple_identifier) @name)) @definition.property
(enum_entry name: (simple_identifier) @name) @definition.constant

;; inheritance and conformance
(class_declaration name: (type_identifier) @name (inheritance_specifier inherits_from: (user_type (type_identifier) @supertype))) @definition.any
(protocol_declaration name: (type_identifier) @name (inheritance_specifier inherits_from: (user_type (type_identifier) @supertype))) @definition.any

;; imports
(import_declaration (identifier (simple_identifier) @name)) @reference.import

;; references
(call_expression (simple_identifier) @name) @reference.call
(call_expression (navigation_expression (navigation_suffix (simple_identifier) @name))) @reference.call
;; a capitalised receiver is a type (or a Kotlin facade such as PlatformKt), worth a reference
((navigation_expression target: (simple_identifier) @name) @reference.type
  (#match? @name "^[A-Z]"))
