;; Haskell for codemap (grammar: tree-sitter-haskell).

(header module: (module) @name) @definition.module
(data_type name: (name) @name) @definition.struct
(newtype (name) @name) @definition.struct
(class (name) @name) @definition.trait
(instance (name) @name) @definition.implementation
(function name: (variable) @name) @definition.function
(signature name: (variable) @name) @definition.function
(declarations (bind name: (variable) @name) @definition.constant)

(apply function: (variable) @name) @reference.call
(import module: (module) @name) @reference.import
