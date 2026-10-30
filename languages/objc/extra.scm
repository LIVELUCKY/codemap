;; Objective-C for codemap (grammar: tree-sitter-objc). A method is named by the first part of its
;; selector.

(protocol_declaration . (identifier) @name) @definition.protocol
(protocol_declaration . (identifier) @name (protocol_reference_list (identifier) @supertype)) @definition.any
(class_interface . (identifier) @name) @definition.class
(class_interface . (identifier) @name superclass: (identifier) @supertype) @definition.any
(class_interface . (identifier) @name (parameterized_arguments (type_name (type_identifier) @supertype))) @definition.any
(class_implementation . (identifier) @name) @definition.implementation
(method_declaration (method_type) . (identifier) @name) @definition.method
(method_definition (method_type) . (identifier) @name) @definition.method
(function_definition declarator: (function_declarator declarator: (identifier) @name)) @definition.function
(message_expression method: (identifier) @name) @reference.call
(call_expression function: (identifier) @name) @reference.call
