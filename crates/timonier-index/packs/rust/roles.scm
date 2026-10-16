; Role signals read in Rust code (contract: ADR 0012). Libraries and imported paths are in
; pack.toml.

; A `main` function at the root of a file is an entry point.
(source_file
  (function_item name: (identifier) @role.entry_point (#eq? @role.entry_point "main")))

; `#[tokio::main]` and friends.
(attribute_item
  (attribute
    (scoped_identifier name: (identifier) @role.entry_point)
    (#eq? @role.entry_point "main")))

; Types that derive Serialize or Deserialize model data.
(attribute_item
  (attribute
    (identifier) @_derive
    arguments: (token_tree (identifier) @role.data_model))
  (#eq? @_derive "derive")
  (#any-of? @role.data_model "Serialize" "Deserialize"))

; Spawning threads or tasks is background work.
(call_expression
  function: (scoped_identifier
    name: (identifier) @role.background_task)
  (#any-of? @role.background_task "spawn" "spawn_blocking"))
