; Role signals read in C code (contract: ADR 0012). Libraries and included headers are in
; pack.toml.

; `int main(…)` is an entry point.
(function_definition
  declarator: (function_declarator declarator: (identifier) @role.entry_point)
  (#eq? @role.entry_point "main"))

; A server that listens: `listen(fd, …)`, `accept(fd, …)`.
(call_expression
  function: (identifier) @role.entry_point
  (#any-of? @role.entry_point "listen" "accept"))

; Threads started by the code.
(call_expression
  function: (identifier) @role.background_task
  (#any-of? @role.background_task "pthread_create" "thrd_create" "CreateThread"))
