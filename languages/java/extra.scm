;; Java definitions and references for codemap (grammar: tree-sitter-java).
;; Based on the grammar's queries/tags.scm (MIT), extended with supertypes, fields and imports.

(class_declaration name: (identifier) @name) @definition.class
(record_declaration name: (identifier) @name) @definition.class
(interface_declaration name: (identifier) @name) @definition.interface
(enum_declaration name: (identifier) @name) @definition.enum
(annotation_type_declaration name: (identifier) @name) @definition.interface
(method_declaration name: (identifier) @name) @definition.method
(constructor_declaration name: (identifier) @name) @definition.method
(field_declaration declarator: (variable_declarator name: (identifier) @name)) @definition.property
(enum_constant name: (identifier) @name) @definition.constant

(class_declaration name: (identifier) @name (superclass (_) @supertype)) @definition.any
(class_declaration name: (identifier) @name (super_interfaces (type_list (_) @supertype))) @definition.any
(interface_declaration name: (identifier) @name (extends_interfaces (type_list (_) @supertype))) @definition.any
(enum_declaration name: (identifier) @name (super_interfaces (type_list (_) @supertype))) @definition.any
(record_declaration name: (identifier) @name (super_interfaces (type_list (_) @supertype))) @definition.any

(import_declaration (scoped_identifier scope: (_) @path name: (identifier) @name)) @reference.import

(method_invocation name: (identifier) @name) @reference.call
(object_creation_expression type: (type_identifier) @name) @reference.new
