; Role signals read in Swift code (contract: ADR 0012). Libraries and imported modules are in
; pack.toml.

; `@main` marks the entry point.
((attribute (user_type (type_identifier) @role.entry_point))
  (#eq? @role.entry_point "main"))

; Types that are encoded and decoded model data; SwiftUI views are an interface.
((inheritance_specifier inherits_from: (user_type (type_identifier) @role.data_model))
  (#any-of? @role.data_model "Codable" "Decodable" "Encodable"))

((inheritance_specifier inherits_from: (user_type (type_identifier) @role.interface))
  (#any-of? @role.interface "View" "App" "UIViewController" "NSViewController"))

; Tasks and threads started by the code.
(call_expression . (simple_identifier) @role.background_task
  (#any-of? @role.background_task "Task" "Thread" "detach"))
