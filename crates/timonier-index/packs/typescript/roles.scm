; Role signals read in TypeScript code (contract: ADR 0012). Libraries and imported paths are
; in pack.toml.

; A server that listens.
(call_expression
  function: (member_expression property: (property_identifier) @role.entry_point)
  (#eq? @role.entry_point "listen"))

; A VSCode extension's entry point.
(export_statement
  declaration: (function_declaration name: (identifier) @role.entry_point)
  (#eq? @role.entry_point "activate"))

; Workers.
(new_expression
  constructor: (identifier) @role.background_task
  (#eq? @role.background_task "Worker"))
