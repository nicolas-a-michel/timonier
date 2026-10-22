; Role signals read in Kotlin code (contract: ADR 0012). Libraries and imported paths are in
; pack.toml.

; `fun main()` of a file is an entry point.
(source_file
  (function_declaration name: (identifier) @role.entry_point (#eq? @role.entry_point "main")))

; Frameworks say what a class is by its annotations.
((annotation (user_type (identifier) @role.entry_point))
  (#any-of? @role.entry_point "SpringBootApplication" "RestController" "Controller" "RequestMapping"))

((annotation (user_type (identifier) @role.persistence))
  (#any-of? @role.persistence "Repository" "Dao" "Database"))

((annotation (user_type (identifier) @role.data_model))
  (#any-of? @role.data_model "Entity" "Serializable" "Table" "Document"))

((annotation (user_type (identifier) @role.background_task))
  (#any-of? @role.background_task "Scheduled" "Async" "KafkaListener"))

; A Ktor server: `embeddedServer(Netty, port = 8080) { … }`.
(call_expression . (identifier) @role.entry_point (#eq? @role.entry_point "embeddedServer"))

; Coroutines and threads started by the code.
(call_expression . (identifier) @role.background_task
  (#any-of? @role.background_task "launch" "async" "thread" "runBlocking"))
