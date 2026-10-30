;; YAML for codemap (grammar: tree-sitter-yaml). Mapping keys are definitions; scalar values that look
;; like paths or URLs are literals.

(block_mapping_pair key: (flow_node (plain_scalar (string_scalar) @name))) @definition.key
(flow_pair key: (flow_node (plain_scalar (string_scalar) @name))) @definition.key
(block_mapping_pair value: (flow_node (plain_scalar (string_scalar) @literal)))
(block_mapping_pair value: (flow_node (double_quote_scalar) @literal))
(block_mapping_pair value: (flow_node (single_quote_scalar) @literal))
(block_sequence_item (flow_node (plain_scalar (string_scalar) @literal)))
(block_sequence_item (flow_node (double_quote_scalar) @literal))
