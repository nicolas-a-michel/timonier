; Role signals read in C++ code (contract: ADR 0012). Libraries and included headers are in
; pack.toml.

; `int main(…)` is an entry point.
(function_definition
  declarator: (function_declarator declarator: (identifier) @role.entry_point)
  (#eq? @role.entry_point "main"))

; A server that listens: `listen(fd, …)`, `acceptor.listen()`, `server.listen(…)`.
(call_expression
  function: [(identifier) (field_expression field: (field_identifier))] @role.entry_point
  (#match? @role.entry_point "(^|[.>])(listen|accept)$"))

; Threads started by the code.
((qualified_identifier) @role.background_task
  (#match? @role.background_task "^std::(thread|jthread|async)$"))
