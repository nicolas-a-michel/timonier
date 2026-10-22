; Role signals read in Python code (contract: ADR 0012). Libraries and imported paths are in
; pack.toml.

; A server that serves: `httpd.serve_forever()`, `uvicorn.run(app)`, `app.run()`.
(call
  function: (attribute attribute: (identifier) @role.entry_point)
  (#any-of? @role.entry_point "serve_forever" "run_forever"))

; A script: `if __name__ == "__main__":`.
(if_statement
  condition: (comparison_operator
    (identifier) @role.entry_point
    (string))
  (#eq? @role.entry_point "__name__"))

; Threads and processes started by the code.
(call
  function: [(identifier) (attribute attribute: (identifier))] @role.background_task
  (#match? @role.background_task "(^|\\.)(Thread|Process|ThreadPoolExecutor|ProcessPoolExecutor)$"))
