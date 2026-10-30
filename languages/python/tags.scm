;; synced from tree-sitter-python-0.25.0 (queries/tags.scm) by scripts/sync-queries; do not edit, put additions in extra.scm
;; licence: see the LICENSE file of that crate
(module (expression_statement (assignment left: (identifier) @name) @definition.constant))

(class_definition
  name: (identifier) @name) @definition.class

(function_definition
  name: (identifier) @name) @definition.function

(call
  function: [
      (identifier) @name
      (attribute
        attribute: (identifier) @name)
  ]) @reference.call
