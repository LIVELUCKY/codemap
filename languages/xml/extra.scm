;; XML for codemap (grammar: tree-sitter-xml). An element with a `name` or `id` attribute is a
;; definition (an Android resource, a view id, a bean); an attribute value that points at a resource
;; (`@drawable/ic_logo`) is a reference to it, so `uses ic_logo` finds the layouts that show the image.

((element [(STag (Attribute (Name) @_attr (AttValue) @name)) (EmptyElemTag (Attribute (Name) @_attr (AttValue) @name))]) @definition.name
  (#eq? @_attr "name"))
((element [(STag (Attribute (Name) @_attr (AttValue) @name)) (EmptyElemTag (Attribute (Name) @_attr (AttValue) @name))]) @definition.id
  (#match? @_attr "^(id|android:id)$"))
((Attribute (Name) @_attr (AttValue) @name) @reference.resource
  (#not-match? @_attr "^(id|android:id)$")
  (#match? @name "^.@[A-Za-z_.:]+/"))
