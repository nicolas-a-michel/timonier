; Imports, calls, references and heritage of the Go pack (contract: ADR 0012). A node captured
; by several patterns keeps the first one, and a reference nested in another reference is
; dropped: the outer path wins.

; `import "example.com/shop/cart"` binds `cart`, the path's last segment; `import c "…"` binds
; `c`; `_` and `.` bind nothing.
(import_spec
  name: (_) @import.alias
  path: (interpreted_string_literal (interpreted_string_literal_content) @import.path)) @import

(import_spec
  !name
  path: (interpreted_string_literal (interpreted_string_literal_content) @import.path)) @import

; Calls: `Total(x)`, and `cart.Total(x)` resolved through the imported package `cart`. A call on
; a value (`c.Total()`) resolves the same way when `c` is an import, never by its name alone.
(call_expression function: (identifier) @reference.call)

(call_expression function: (selector_expression) @reference.method)

; Embedding a type in a struct or an interface is Go's heritage (before the types, which would
; take these nodes): `type Shop struct { cart.Store }`.
(type_spec
  name: (_) @reference.source
  type: (struct_type
    (field_declaration_list
      (field_declaration
        !name
        type: [(type_identifier) (qualified_type)] @reference.inherits))))

(type_spec
  name: (_) @reference.source
  type: (interface_type
    (type_elem [(type_identifier) (qualified_type)] @reference.inherits)))

; Types: `Item`, `cart.Item`, in declarations, conversions and literals.
(qualified_type) @reference.type

(type_identifier) @reference.type
