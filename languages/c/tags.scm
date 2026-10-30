;; synced from tree-sitter-c-0.24.2 (queries/tags.scm) by scripts/sync-queries; do not edit, put additions in extra.scm
;; licence: see the LICENSE file of that crate
(struct_specifier name: (type_identifier) @name body:(_)) @definition.class

(declaration type: (union_specifier name: (type_identifier) @name)) @definition.class

(function_declarator declarator: (identifier) @name) @definition.function

(type_definition declarator: (type_identifier) @name) @definition.type

(enum_specifier name: (type_identifier) @name) @definition.type
