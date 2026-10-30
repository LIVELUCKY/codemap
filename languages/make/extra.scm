;; Make for codemap (grammar: tree-sitter-make). Targets, variables and `define` blocks are
;; definitions; prerequisites and variable expansions are references.

(rule (targets (word) @name)) @definition.target
(variable_assignment name: (word) @name) @definition.variable
(define_directive name: (word) @name) @definition.function
(prerequisites (word) @name) @reference.target
(variable_reference (word) @name) @reference.variable
