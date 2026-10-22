; Definitions of the Swift pack (contract: ADR 0012). When several patterns capture the same
; node, the first one wins.

; An extension contains the members it adds to its type: they are ranged under the type when
; the type is in the same file (ADR 0067).
((class_declaration
   declaration_kind: "extension"
   name: (user_type (type_identifier) @scope.type)) @scope
  (#set! timonier.segment "{scope.type}")
  (#set! timonier.parent "{scope.type}"))

; Classes, actors, structures and enumerations.
(class_declaration name: (type_identifier) @name body: (_) @body) @definition.type

(protocol_declaration name: (_) @name body: (_) @body) @definition.trait

(protocol_function_declaration name: (simple_identifier) @name) @definition.method

(protocol_property_declaration name: (pattern bound_identifier: (_) @name)) @definition.field

(typealias_declaration name: (type_identifier) @name) @definition.type

; `case small, large`: one constant per name.
(enum_entry name: (simple_identifier) @name @definition.constant)

; Methods, initialisers and properties of a type or an extension.
(class_body
  (function_declaration name: (simple_identifier) @name body: (_) @body) @definition.method)

(enum_class_body
  (function_declaration name: (simple_identifier) @name body: (_) @body) @definition.method)

(class_body (init_declaration name: _ @name body: (_) @body) @definition.method)

(class_body
  (property_declaration name: (pattern bound_identifier: (_) @name)) @definition.field)

(enum_class_body
  (property_declaration name: (pattern bound_identifier: (_) @name)) @definition.field)

; Functions and values of a file.
(function_declaration name: (simple_identifier) @name body: (_) @body) @definition.function

(source_file
  (property_declaration name: (pattern bound_identifier: (_) @name)) @definition.constant)
