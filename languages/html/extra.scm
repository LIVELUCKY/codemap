;; HTML for codemap (grammar: tree-sitter-html). Element ids are definitions; href and src values are
;; literals.

((element (start_tag (attribute (attribute_name) @_attr (quoted_attribute_value (attribute_value) @name)))) @definition.id
  (#eq? @_attr "id"))
((attribute (attribute_name) @_attr (quoted_attribute_value (attribute_value) @literal))
  (#match? @_attr "^(href|src|action|data-src)$"))
