;; Rust definitions and references for codemap (grammar: tree-sitter-rust).
;; Based on the grammar's queries/tags.scm (MIT), with impl blocks as definitions carrying the trait
;; as supertype, constants, and use declarations as imports.

(struct_item name: (type_identifier) @name) @definition.struct
(enum_item name: (type_identifier) @name) @definition.enum
(union_item name: (type_identifier) @name) @definition.struct
(type_item name: (type_identifier) @name) @definition.typealias
(trait_item name: (type_identifier) @name) @definition.trait
(mod_item name: (identifier) @name) @definition.module
(macro_definition name: (identifier) @name) @definition.macro
(declaration_list (function_item name: (identifier) @name) @definition.method)
(declaration_list (function_signature_item name: (identifier) @name) @definition.method)
(source_file (function_item name: (identifier) @name) @definition.function)
(const_item name: (identifier) @name) @definition.constant
(static_item name: (identifier) @name) @definition.constant

;; `impl Trait for Type` is a definition named after the type, with the trait as supertype
(impl_item trait: (type_identifier) @supertype type: (type_identifier) @name) @definition.implementation
(impl_item trait: (generic_type type: (type_identifier) @supertype) type: (type_identifier) @name) @definition.implementation
(impl_item type: (type_identifier) @name !trait) @definition.implementation
(impl_item type: (generic_type type: (type_identifier) @name) !trait) @definition.implementation

(use_declaration argument: (scoped_identifier path: (_) @path name: (identifier) @name)) @reference.import
(use_declaration argument: (scoped_use_list path: (_) @path list: (use_list (identifier) @name))) @reference.import

(call_expression function: (identifier) @name) @reference.call
(call_expression function: (field_expression field: (field_identifier) @name)) @reference.call
(call_expression function: (scoped_identifier name: (identifier) @name)) @reference.call
(macro_invocation macro: (identifier) @name) @reference.call
