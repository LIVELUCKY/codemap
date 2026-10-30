;; Python definitions and references for codemap (grammar: tree-sitter-python).
;; Based on the grammar's queries/tags.scm (MIT), extended with methods, bases and imports.

;; headers
(class_definition name: (identifier) @name) @definition.class
(module (function_definition name: (identifier) @name) @definition.function)
(module (decorated_definition definition: (function_definition name: (identifier) @name) @definition.function))
;; the class name is captured too, so these outrank upstream's plain function pattern for methods
(class_definition name: (identifier) @_class body: (block (function_definition name: (identifier) @name) @definition.method))
(class_definition name: (identifier) @_class body: (block (decorated_definition definition: (function_definition name: (identifier) @name) @definition.method)))
(module (expression_statement (assignment left: (identifier) @name) @definition.constant))

;; bases
(class_definition name: (identifier) @name superclasses: (argument_list (identifier) @supertype)) @definition.any
(class_definition name: (identifier) @name superclasses: (argument_list (attribute attribute: (identifier) @supertype))) @definition.any

;; imports: the imported name, with the module as the flag
(import_from_statement module_name: (dotted_name) @path name: (dotted_name (identifier) @name)) @reference.import
(import_from_statement module_name: (dotted_name) @path name: (aliased_import name: (dotted_name (identifier) @name))) @reference.import
(import_statement name: (dotted_name (identifier) @name)) @reference.import

;; references
(call function: (identifier) @name) @reference.call
(call function: (attribute attribute: (identifier) @name)) @reference.call
