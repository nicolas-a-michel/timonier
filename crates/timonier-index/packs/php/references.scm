; Uses, calls, references and heritage of the PHP pack (contract: ADR 0012). A node captured by
; several patterns keeps the first one, and a reference nested in another reference is
; dropped: the outer path wins.

; `use Shop\Model\Item;` binds `Item`; `use Shop\Model\Item as Line;` binds `Line`.
(namespace_use_clause [(qualified_name) (name)] @import.path alias: (name) @import.alias) @import

(namespace_use_clause . [(qualified_name) (name)] @import.path .) @import

; Heritage first, since the names below would take these nodes.
(class_declaration
  name: (_) @reference.source
  (base_clause [(name) (qualified_name)] @reference.inherits))

(class_declaration
  name: (_) @reference.source
  (class_interface_clause [(name) (qualified_name)] @reference.implements))

(interface_declaration
  name: (_) @reference.source
  (base_clause [(name) (qualified_name)] @reference.inherits))

; Calls and creations: `cart_total()`, `new Cart()`, `Cart::make()` through its class; a call
; on an object (`$cart->total()`) never by its name alone (ADR 0013).
(function_call_expression function: [(name) (qualified_name)] @reference.call)

(scoped_call_expression scope: [(name) (qualified_name)] @reference.call)

(object_creation_expression [(name) (qualified_name)] @reference.call)

(class_constant_access_expression . [(name) (qualified_name)] @reference.call)

; Types in parameters, properties and returns, and attributes.
(named_type [(name) (qualified_name)] @reference.type)

(attribute [(name) (qualified_name)] @reference.type)
