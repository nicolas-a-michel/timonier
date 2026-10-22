; Usings, calls, references and heritage of the C# pack (contract: ADR 0012). A node captured
; by several patterns keeps the first one, and a reference nested in another reference is
; dropped: the outer path wins.

; `using Shop.Model;`, `using static Shop.Prices;`: a namespace, whose names are then seen; a
; name is resolved in the repository (ADR 0069). `global using` alike.
(using_directive [(qualified_name) (identifier)] @import.path) @import

; Heritage first, since the types below would take these nodes: a base class and the
; interfaces of a type, which C# writes in one list.
(class_declaration
  name: (_) @reference.source
  (base_list [(identifier) (qualified_name) (generic_name)] @reference.inherits))

(record_declaration
  name: (_) @reference.source
  (base_list [(identifier) (qualified_name) (generic_name)] @reference.inherits))

(struct_declaration
  name: (_) @reference.source
  (base_list [(identifier) (qualified_name) (generic_name)] @reference.implements))

(interface_declaration
  name: (_) @reference.source
  (base_list [(identifier) (qualified_name) (generic_name)] @reference.inherits))

; Calls and creations: `Total(c)`, `new Cart()`; `Cart.Make()` through its first name, which
; resolves when it is a type.
(invocation_expression function: (identifier) @reference.call)

(member_access_expression expression: (identifier) @reference.call)

(object_creation_expression type: [(identifier) (qualified_name) (generic_name)] @reference.call)

; Attributes: `[Fact]`, `[ApiController]`.
(attribute name: (_) @reference.type)

; Types in declarations, parameters, generics and casts.
(generic_name . (identifier) @reference.type)

(variable_declaration type: (identifier) @reference.type)

(parameter type: (identifier) @reference.type)

(method_declaration returns: (identifier) @reference.type)

(property_declaration type: (identifier) @reference.type)

(type_argument_list (identifier) @reference.type)
