;; Zig for codemap (grammar: tree-sitter-zig). Containers are `const Name = struct { ... }`; the
;; container patterns capture the body too, which makes them more specific than the plain constant
;; pattern and wins them the kind.

(function_declaration name: (identifier) @name) @definition.function
(variable_declaration (identifier) @name (struct_declaration) @_body) @definition.struct
(variable_declaration (identifier) @name (enum_declaration) @_body) @definition.enum
(variable_declaration (identifier) @name (union_declaration) @_body) @definition.struct
(variable_declaration (identifier) @name) @definition.constant

(call_expression function: (identifier) @name) @reference.call
(call_expression function: (field_expression member: (identifier) @name)) @reference.call
((builtin_function (builtin_identifier) @_b (arguments (string) @name)) @reference.import
  (#eq? @_b "@import"))
(string) @literal
