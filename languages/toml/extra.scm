;; TOML for codemap (grammar: tree-sitter-toml-ng). Keys and table headers are definitions; string
;; values that look like paths or URLs are literals.

(pair (bare_key) @name) @definition.key
(pair (quoted_key) @name) @definition.key
(pair (dotted_key) @name) @definition.key
(table (bare_key) @name) @definition.key
(table (dotted_key) @name) @definition.key
(table_array_element (bare_key) @name) @definition.key
(table_array_element (dotted_key) @name) @definition.key
(pair (string) @literal)
(array (string) @literal)
