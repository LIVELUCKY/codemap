;; Elixir for codemap (grammar: tree-sitter-elixir). Based on the grammar's queries/tags.scm (Apache-2.0),
;; extended with module attributes as constants.

(call
  target: (identifier) @_def
  (arguments (alias) @name)
  (#any-of? @_def "defmodule" "defprotocol")) @definition.module

(call
  target: (identifier) @_def
  (arguments
    [
      (identifier) @name
      (call target: (identifier) @name)
      (binary_operator left: (call target: (identifier) @name) operator: "when")
    ])
  (#any-of? @_def "def" "defp" "defdelegate" "defguard" "defguardp" "defmacro" "defmacrop" "defn" "defnp")) @definition.function

(unary_operator operator: "@" operand: (call target: (identifier) @name)) @definition.constant

((call target: [(identifier) @name (dot right: (identifier) @name)]) @reference.call
  (#not-match? @name "^(def|defp|defmodule|defprotocol|defimpl|defstruct|defexception|defdelegate|defguard|defguardp|defmacro|defmacrop|defn|defnp|alias|import|require|use|if|unless|case|cond|with|for|quote|raise|receive|try)$"))
(binary_operator operator: "|>" right: (identifier) @name) @reference.call
(string (quoted_content) @literal)
