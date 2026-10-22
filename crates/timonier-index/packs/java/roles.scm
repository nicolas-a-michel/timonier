; Role signals read in Java code (contract: ADR 0012). Libraries and imported paths are in
; pack.toml.

; `public static void main(String[] args)` is an entry point.
(method_declaration name: (identifier) @role.entry_point (#eq? @role.entry_point "main"))

; Frameworks say what a class is by its annotations.
((marker_annotation name: (identifier) @role.entry_point)
  (#any-of? @role.entry_point
    "SpringBootApplication" "RestController" "Controller" "WebServlet" "QuarkusMain"))

((annotation name: (identifier) @role.entry_point)
  (#any-of? @role.entry_point "RequestMapping" "Path" "WebServlet" "Controller"))

((marker_annotation name: (identifier) @role.persistence)
  (#any-of? @role.persistence "Repository" "Mapper"))

((marker_annotation name: (identifier) @role.data_model)
  (#any-of? @role.data_model "Entity" "Embeddable" "Table" "Document"))

((annotation name: (identifier) @role.data_model)
  (#any-of? @role.data_model "Entity" "Table" "Document"))

((marker_annotation name: (identifier) @role.background_task)
  (#any-of? @role.background_task "Async" "Scheduled"))

((annotation name: (identifier) @role.background_task)
  (#any-of? @role.background_task "Scheduled" "KafkaListener" "RabbitListener" "JmsListener"))

; A server that listens: `new ServerSocket(…)`, `HttpServer.create(…)`.
(object_creation_expression
  type: (type_identifier) @role.entry_point
  (#eq? @role.entry_point "ServerSocket"))

(method_invocation
  object: (identifier) @_server
  name: (identifier) @role.entry_point
  (#eq? @_server "HttpServer")
  (#eq? @role.entry_point "create"))

; Threads and executors started by the code.
(object_creation_expression
  type: (type_identifier) @role.background_task
  (#eq? @role.background_task "Thread"))

(method_invocation
  object: (identifier) @_executors
  name: (identifier) @role.background_task
  (#eq? @_executors "Executors"))
