; Role signals read in C# code (contract: ADR 0012). Libraries and usings are in pack.toml.

; `static void Main(…)` is an entry point.
(method_declaration name: (identifier) @role.entry_point (#eq? @role.entry_point "Main"))

; ASP.NET Core: `WebApplication.CreateBuilder(…)`, controllers.
(member_access_expression
  expression: (identifier) @_app
  name: (identifier) @role.entry_point
  (#eq? @_app "WebApplication")
  (#eq? @role.entry_point "CreateBuilder"))

((attribute name: (identifier) @role.entry_point)
  (#any-of? @role.entry_point "ApiController" "Route" "HttpGet" "HttpPost"))

; Data models and persistence: entities, an Entity Framework context.
((attribute name: (identifier) @role.data_model)
  (#any-of? @role.data_model "Table" "Key" "JsonSerializable" "DataContract"))

(class_declaration
  (base_list (identifier) @role.persistence)
  (#eq? @role.persistence "DbContext"))

; Background work: hosted services, tasks and threads started by the code.
(class_declaration
  (base_list (identifier) @role.background_task)
  (#any-of? @role.background_task "BackgroundService" "IHostedService"))

(member_access_expression
  expression: (identifier) @_task
  name: (identifier) @role.background_task
  (#eq? @_task "Task")
  (#eq? @role.background_task "Run"))
