;; Dart for codemap (grammar: tree-sitter-dart). Based on the grammar's queries/tags.scm (MIT),
;; extended with extends and implements as supertypes.

(class_declaration name: (identifier) @name) @definition.class
(mixin_declaration (identifier) @name) @definition.class
(extension_declaration name: (identifier) @name) @definition.extension
(enum_declaration name: (identifier) @name) @definition.enum
(enum_entry name: (identifier) @name) @definition.constant
(function_signature name: (identifier) @name) @definition.function
(getter_signature name: (identifier) @name) @definition.function
(setter_signature name: (identifier) @name) @definition.function
(method_signature (function_signature name: (identifier) @name)) @definition.method
(method_signature (getter_signature name: (identifier) @name)) @definition.method
(method_signature (setter_signature name: (identifier) @name)) @definition.method
(constructor_signature name: (identifier) @name) @definition.method
(factory_constructor_signature (identifier) @name) @definition.method
(initialized_identifier name: (identifier) @name) @definition.property
(static_final_declaration name: (identifier) @name) @definition.property
(type_alias (type_identifier) @name) @definition.typealias

(class_declaration name: (identifier) @name (superclass (type (type_identifier) @supertype))) @definition.any
(class_declaration name: (identifier) @name (interfaces (type (type_identifier) @supertype))) @definition.any
(class_declaration name: (identifier) @name (superclass (mixins (type (type_identifier) @supertype)))) @definition.any

(import_or_export (library_import (import_specification uri: (configurable_uri (uri (string_literal) @name))))) @reference.import
(import_or_export (library_import (import_specification uri: (uri (string_literal) @name)))) @reference.import

(call_expression function: (identifier) @name) @reference.call
(call_expression function: (member_expression property: (identifier) @name)) @reference.call
(new_expression type: (type (type_identifier) @name)) @reference.new
(string_literal) @literal
