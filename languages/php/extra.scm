;; PHP definitions and references for codemap (grammar: tree-sitter-php).
;; Based on the grammar's queries/tags.scm (MIT), extended with extends and implements as supertypes,
;; enums, constants and methods as their own kind.

(namespace_definition name: (namespace_name) @name) @definition.module
(interface_declaration name: (name) @name) @definition.interface
(trait_declaration name: (name) @name) @definition.trait
(class_declaration name: (name) @name) @definition.class
(enum_declaration name: (name) @name) @definition.enum
(function_definition name: (name) @name) @definition.function
(method_declaration name: (name) @name) @definition.method
(property_declaration (property_element (variable_name (name) @name))) @definition.property
(const_declaration (const_element (name) @name)) @definition.constant

(class_declaration name: (name) @name (base_clause (_) @supertype)) @definition.any
(class_declaration name: (name) @name (class_interface_clause (_) @supertype)) @definition.any
(interface_declaration name: (name) @name (base_clause (_) @supertype)) @definition.any

(namespace_use_declaration (namespace_use_clause (qualified_name (name) @name))) @reference.import
(namespace_use_declaration (namespace_use_clause (name) @name)) @reference.import

(function_call_expression function: [(name) @name (qualified_name (name) @name)]) @reference.call
(scoped_call_expression name: (name) @name) @reference.call
(member_call_expression name: (name) @name) @reference.call
(object_creation_expression [(name) @name (qualified_name (name) @name)]) @reference.new
