;; Kotlin definitions and references for codemap (grammar: tree-sitter-kotlin-ng).
;; Headers give the kind; the `any` patterns add modifiers, annotations and supertypes to the same
;; definition and are merged by name and line.

;; headers
(class_declaration "class" name: (identifier) @name) @definition.class
(class_declaration "interface" name: (identifier) @name) @definition.interface
(object_declaration name: (identifier) @name) @definition.object
(function_declaration name: (identifier) @name) @definition.function
(type_alias type: (identifier) @name) @definition.typealias
(enum_entry (identifier) @name) @definition.constant
(class_body (property_declaration (variable_declaration (identifier) @name)) @definition.property)
(source_file (property_declaration (variable_declaration (identifier) @name)) @definition.property)

;; modifiers
(class_declaration
  (modifiers [(class_modifier) (platform_modifier) (inheritance_modifier) (member_modifier) (visibility_modifier)] @modifier)
  name: (identifier) @name) @definition.any
(object_declaration
  (modifiers [(class_modifier) (platform_modifier) (inheritance_modifier) (member_modifier) (visibility_modifier)] @modifier)
  name: (identifier) @name) @definition.any
(function_declaration
  (modifiers [(function_modifier) (platform_modifier) (inheritance_modifier) (member_modifier) (visibility_modifier)] @modifier)
  name: (identifier) @name) @definition.any
(property_declaration
  (modifiers [(property_modifier) (platform_modifier) (inheritance_modifier) (member_modifier) (visibility_modifier)] @modifier)
  (variable_declaration (identifier) @name)) @definition.any

;; annotations
(class_declaration (modifiers (annotation) @annotation) name: (identifier) @name) @definition.any
(object_declaration (modifiers (annotation) @annotation) name: (identifier) @name) @definition.any
(function_declaration (modifiers (annotation) @annotation) name: (identifier) @name) @definition.any
(property_declaration (modifiers (annotation) @annotation) (variable_declaration (identifier) @name)) @definition.any

;; supertypes
(class_declaration name: (identifier) @name (delegation_specifiers (delegation_specifier) @supertype)) @definition.any
(object_declaration name: (identifier) @name (delegation_specifiers (delegation_specifier) @supertype)) @definition.any

;; annotations with arguments after a bodiless declaration parse as an expression statement;
;; the definition on the next line adopts them (see grammar.rs)
(annotated_expression (annotation) @orphan.annotation) @orphan

;; references
(call_expression (identifier) @name) @reference.call
(call_expression (navigation_expression (identifier) @name .)) @reference.call
;; a name used as a type: `val x: CartViewModel`, `inject<CartViewModel>()`, `: Base`
(user_type (identifier) @name) @reference.type
;; the receiver of a member access and the member read: `AppTheme.colors`, `R.drawable.ic_logo`
(navigation_expression . (identifier) @name) @reference.read
(navigation_expression (identifier) @name .) @reference.read
;; `::CartViewModel`
(callable_reference (identifier) @name) @reference.call
