; Imports, calls, references and heritage of the Swift pack (contract: ADR 0012). A node
; captured by several patterns keeps the first one, and a reference nested in another
; reference is dropped: the outer path wins.

; `import Shop`: a module, a target of the package or a dependency's product. Every name of
; the module is then seen; a name is resolved in the package (ADR 0067).
(import_declaration (identifier) @import.path) @import

; Heritage first, since the types below would take these nodes: a superclass and the
; protocols a type conforms to.
(class_declaration
  name: (_) @reference.source
  (inheritance_specifier inherits_from: (user_type (type_identifier) @reference.inherits)))

(protocol_declaration
  name: (_) @reference.source
  (inheritance_specifier inherits_from: (user_type (type_identifier) @reference.inherits)))

; Calls and constructions: `total()`, `Cart()`; `Cart.make()` through its first name, which
; resolves when it is a type.
(call_expression . (simple_identifier) @reference.call)

(navigation_expression target: (simple_identifier) @reference.call)

; Types, in declarations, generics and attributes.
(user_type (type_identifier) @reference.type)
