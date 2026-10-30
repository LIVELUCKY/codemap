;; Ruby definitions and references for codemap (grammar: tree-sitter-ruby).
;; Based on the grammar's queries/tags.scm (MIT), without the doc-comment and locals predicates,
;; extended with superclasses, constants and requires.

(class name: [(constant) @name (scope_resolution name: (_) @name)]) @definition.class
(singleton_class value: [(constant) @name (scope_resolution name: (_) @name)]) @definition.class
(module name: [(constant) @name (scope_resolution name: (_) @name)]) @definition.module
(method name: (_) @name) @definition.method
(singleton_method name: (_) @name) @definition.method
(alias name: (_) @name) @definition.method
(program (assignment left: (constant) @name) @definition.constant)

(class name: (constant) @name superclass: (superclass (constant) @supertype)) @definition.any

;; require "x" and require_relative "x"
((call method: (identifier) @_req arguments: (argument_list (string (string_content) @name))) @reference.import
  (#match? @_req "^require(_relative)?$"))

(call method: (identifier) @name) @reference.call
;; a bare identifier on its own line inside a method body is a call when it is not a local
(body_statement (identifier) @name @reference.call)
