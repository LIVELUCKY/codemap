;; C# definitions and references for codemap (grammar: tree-sitter-c-sharp).
;; Based on the grammar's queries/tags.scm (MIT), extended with base lists as supertypes, structs,
;; enums, properties, fields and usings.

(namespace_declaration name: (_) @name) @definition.module
(file_scoped_namespace_declaration name: (_) @name) @definition.module
(class_declaration name: (identifier) @name) @definition.class
(record_declaration name: (identifier) @name) @definition.class
(struct_declaration name: (identifier) @name) @definition.struct
(interface_declaration name: (identifier) @name) @definition.interface
(enum_declaration name: (identifier) @name) @definition.enum
(method_declaration name: (identifier) @name) @definition.method
(constructor_declaration name: (identifier) @name) @definition.method
(property_declaration name: (identifier) @name) @definition.property
(field_declaration (variable_declaration (variable_declarator (identifier) @name))) @definition.property
(enum_member_declaration name: (identifier) @name) @definition.constant

(class_declaration name: (identifier) @name (base_list (_) @supertype)) @definition.any
(struct_declaration name: (identifier) @name (base_list (_) @supertype)) @definition.any
(interface_declaration name: (identifier) @name (base_list (_) @supertype)) @definition.any
(record_declaration name: (identifier) @name (base_list (_) @supertype)) @definition.any

(using_directive (_) @name) @reference.import

(invocation_expression function: (identifier) @name) @reference.call
(invocation_expression function: (member_access_expression name: (identifier) @name)) @reference.call
(object_creation_expression type: (identifier) @name) @reference.new
