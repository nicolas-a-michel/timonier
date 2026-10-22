; Role signals read in Swift code (contract: ADR 0012). Libraries and imported modules are in
; pack.toml.

; `@main` marks the entry point.
((attribute (user_type (type_identifier) @role.entry_point))
  (#eq? @role.entry_point "main"))

; Routes declared on an application or a router (Vapor, Hummingbird): `app.get(…)`,
; `routes.grouped(…)`, and route collections.
(call_expression
  (navigation_expression
    suffix: (navigation_suffix suffix: (simple_identifier) @role.entry_point))
  (#any-of? @role.entry_point "get" "post" "put" "patch" "delete" "grouped" "register"))

((inheritance_specifier inherits_from: (user_type (type_identifier) @role.entry_point))
  (#any-of? @role.entry_point "RouteCollection" "AsyncMiddleware" "Middleware"))

; Types that are encoded and decoded model data (Vapor's `Content` too); Fluent's models are
; persistence; SwiftUI views are an interface.
((inheritance_specifier inherits_from: (user_type (type_identifier) @role.data_model))
  (#any-of? @role.data_model "Codable" "Decodable" "Encodable" "Content"))

((inheritance_specifier inherits_from: (user_type (type_identifier) @role.persistence))
  (#any-of? @role.persistence "Model" "Migration" "AsyncMigration"))

((inheritance_specifier inherits_from: (user_type (type_identifier) @role.interface))
  (#any-of? @role.interface "View" "App" "UIViewController" "NSViewController"))

; Tasks and threads started by the code.
(call_expression . (simple_identifier) @role.background_task
  (#any-of? @role.background_task "Task" "Thread" "detach"))
