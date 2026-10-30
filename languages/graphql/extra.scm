;; GraphQL for codemap (grammar: tree-sitter-graphql). Schema types, operations and fragments are
;; definitions; a named type and a fragment spread are references.

(object_type_definition (name) @name) @definition.type
(object_type_definition (name) @name (implements_interfaces (named_type (name) @supertype))) @definition.any
(interface_type_definition (name) @name) @definition.interface
(enum_type_definition (name) @name) @definition.enum
(input_object_type_definition (name) @name) @definition.input
(scalar_type_definition (name) @name) @definition.scalar
(union_type_definition (name) @name) @definition.union
(operation_definition (name) @name) @definition.operation
(fragment_definition (fragment_name (name) @name)) @definition.fragment
(named_type (name) @name) @reference.type
(fragment_spread (fragment_name (name) @name)) @reference.fragment
