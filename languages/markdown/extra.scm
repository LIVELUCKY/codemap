;; Markdown for codemap (grammar: tree-sitter-md, block level). Headings are definitions; link
;; targets are found by a line scan in parse/mod.rs because links live in the inline grammar.

(atx_heading heading_content: (inline) @name) @definition.heading
(setext_heading heading_content: (paragraph (inline) @name)) @definition.heading
