;; Scala for codemap (grammar: tree-sitter-scala). Based on the grammar's queries/tags.scm (MIT),
;; extended with extends clauses as supertypes.

(package_clause name: (package_identifier) @name) @definition.module
(trait_definition name: (identifier) @name) @definition.trait
(enum_definition name: (identifier) @name) @definition.enum
(simple_enum_case name: (identifier) @name) @definition.constant
(full_enum_case name: (identifier) @name) @definition.constant
(class_definition name: (identifier) @name) @definition.class
(object_definition name: (identifier) @name) @definition.object
(function_definition name: (identifier) @name) @definition.function
(function_declaration name: (identifier) @name) @definition.function
(val_definition pattern: (identifier) @name) @definition.property
(var_definition pattern: (identifier) @name) @definition.property
(val_declaration name: (identifier) @name) @definition.property
(var_declaration name: (identifier) @name) @definition.property
(given_definition name: (identifier) @name) @definition.property
(type_definition name: (type_identifier) @name) @definition.typealias

(class_definition name: (identifier) @name (extends_clause (type_identifier) @supertype)) @definition.any
(class_definition name: (identifier) @name (extends_clause (generic_type (type_identifier) @supertype))) @definition.any
(object_definition name: (identifier) @name (extends_clause (type_identifier) @supertype)) @definition.any
(trait_definition name: (identifier) @name (extends_clause (type_identifier) @supertype)) @definition.any

(import_declaration (namespace_selectors (identifier) @name)) @reference.import

(call_expression (identifier) @name) @reference.call
(call_expression (field_expression field: (identifier) @name)) @reference.call
(instance_expression (type_identifier) @name) @reference.new
