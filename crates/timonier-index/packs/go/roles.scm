; Role signals read in Go code (contract: ADR 0012). Libraries and imported paths are in
; pack.toml.

; `func main()` is an entry point.
(source_file
  (function_declaration name: (identifier) @role.entry_point (#eq? @role.entry_point "main")))

; A server that listens: `http.ListenAndServe(…)`, `srv.Serve(l)`, `net.Listen(…)`.
(call_expression
  function: (selector_expression field: (field_identifier) @role.entry_point)
  (#any-of? @role.entry_point "ListenAndServe" "ListenAndServeTLS" "Serve" "Listen"))

; A goroutine is background work.
(go_statement) @role.background_task
