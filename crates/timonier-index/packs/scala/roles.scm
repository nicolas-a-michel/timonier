; Role signals read in Scala code (contract: ADR 0012). Libraries and imported paths are in
; pack.toml.

; `@main def run()` and `def main(args: Array[String])` are entry points.
((annotation name: (type_identifier) @role.entry_point)
  (#eq? @role.entry_point "main"))

(function_definition name: (identifier) @role.entry_point (#eq? @role.entry_point "main"))

; Futures and actors started by the code.
(call_expression
  function: (identifier) @role.background_task
  (#any-of? @role.background_task "Future" "spawn"))
