;; HCL and Terraform for codemap (grammar: tree-sitter-hcl). A block is a definition named by its last
;; label (`resource "local_file" "assets"` is `assets`), with the block type as a flag.

(block (identifier) @modifier (string_lit (template_literal) @name) . (block_start)) @definition.block
(variable_expr (identifier) @name) @reference.variable
(get_attr (identifier) @name) @reference.attribute
