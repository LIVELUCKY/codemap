;; Bash for codemap (grammar: tree-sitter-bash).

(function_definition name: (word) @name) @definition.function
(program (variable_assignment name: (variable_name) @name) @definition.property)
(command name: (command_name (word) @name)) @reference.call
(string (string_content) @literal)
(raw_string) @literal
