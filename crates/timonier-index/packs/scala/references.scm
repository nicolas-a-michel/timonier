; Imports, calls, references and heritage of the Scala pack (contract: ADR 0012). A node
; captured by several patterns keeps the first one, and a reference nested in another
; reference is dropped: the outer path wins.

; `import com.shop.Cart` binds `Cart`: its last segment, the others its prefix (ADR 0071).
; A field and an anchor together match nothing in this grammar: the anchor alone.
(import_declaration (identifier) @import.path .) @import

; `import com.shop.{Cart, Price => P}` binds `Cart` and `P`.
(import_declaration (namespace_selectors (identifier) @import.path)) @import

(import_declaration
  (namespace_selectors (arrow_renamed_identifier name: (_) @import.path alias: (_) @import.alias))) @import

; `import com.shop.*` (and Scala 2's `_`) binds no name: the names it brings are looked up in
; that package.
(import_declaration (identifier) @import.path . (namespace_wildcard) @import.alias) @import

; Heritage first, since the types below would take these nodes.
(class_definition
  name: (_) @reference.source
  extend: (extends_clause type: [(type_identifier) (stable_type_identifier) (generic_type)] @reference.inherits))

(object_definition
  name: (_) @reference.source
  extend: (extends_clause type: [(type_identifier) (stable_type_identifier) (generic_type)] @reference.inherits))

(trait_definition
  name: (_) @reference.source
  extend: (extends_clause type: [(type_identifier) (stable_type_identifier) (generic_type)] @reference.inherits))

; Calls and creations: `total(c)`, `new Cart`, `Cart.make()` through its first name, which
; resolves when it is a class or an object.
(call_expression function: (identifier) @reference.call)

(field_expression value: (identifier) @reference.call)

(instance_expression [(type_identifier) (stable_type_identifier) (generic_type)] @reference.call)

; Types in declarations, parameters and generics.
(type_identifier) @reference.type
