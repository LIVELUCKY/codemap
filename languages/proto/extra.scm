;; Protocol Buffers for codemap (grammar: tree-sitter-proto).

(message (message_name (identifier) @name)) @definition.message
(enum (enum_name (identifier) @name)) @definition.enum
(service (service_name (identifier) @name)) @definition.service
(rpc (rpc_name (identifier) @name)) @definition.rpc
(message_or_enum_type (identifier) @name) @reference.type
