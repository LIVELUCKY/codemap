;; Go definitions and references for codemap (grammar: tree-sitter-go).
;; Based on the grammar's queries/tags.scm (MIT), without the doc-comment predicates.

;; the type body is captured too, so these outrank upstream's generic `type` kind
(type_declaration (type_spec name: (type_identifier) @name type: (interface_type) @_body) @definition.interface)
(type_declaration (type_spec name: (type_identifier) @name type: (struct_type) @_body) @definition.struct)
(type_declaration (type_spec name: (type_identifier) @name type: [(type_identifier) (qualified_type) (pointer_type) (map_type) (slice_type) (array_type) (function_type) (channel_type)]) @definition.typealias)
(function_declaration name: (identifier) @name) @definition.function
(method_declaration name: (field_identifier) @name) @definition.method
(const_declaration (const_spec name: (identifier) @name) @definition.constant)
(source_file (var_declaration (var_spec name: (identifier) @name) @definition.property))

(import_declaration (import_spec path: (interpreted_string_literal) @name)) @reference.import
(import_declaration (import_spec_list (import_spec path: (interpreted_string_literal) @name) @reference.import))

(call_expression
  function: [
    (identifier) @name
    (parenthesized_expression (identifier) @name)
    (selector_expression field: (field_identifier) @name)
    (parenthesized_expression (selector_expression field: (field_identifier) @name))
  ]) @reference.call
