;; JSON for codemap (grammar: tree-sitter-json). Keys are definitions; string values that look like
;; paths or URLs are literals.

(pair key: (string (string_content) @name)) @definition.key
(pair value: (string (string_content) @literal))
(array (string (string_content) @literal))
