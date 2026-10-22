; Imports, calls, references and heritage of the Java pack (contract: ADR 0012). A node
; captured by several patterns keeps the first one, and a reference nested in another
; reference is dropped: the outer path wins.

; `import com.shop.Cart;` binds `Cart`; `import static com.shop.Util.max;` binds `max`.
(import_declaration [(scoped_identifier) (identifier)] @import.path .) @import

; `import com.shop.*;` binds no name: the names it brings are looked up in that package.
(import_declaration (scoped_identifier) @import.path (asterisk) @import.alias) @import

; Heritage first, since the types below would take these nodes.
(class_declaration
  name: (_) @reference.source
  superclass: (superclass [(type_identifier) (scoped_type_identifier) (generic_type)] @reference.inherits))

(class_declaration
  name: (_) @reference.source
  interfaces: (super_interfaces
    (type_list [(type_identifier) (scoped_type_identifier) (generic_type)] @reference.implements)))

(record_declaration
  name: (_) @reference.source
  interfaces: (super_interfaces
    (type_list [(type_identifier) (scoped_type_identifier) (generic_type)] @reference.implements)))

(enum_declaration
  name: (_) @reference.source
  interfaces: (super_interfaces
    (type_list [(type_identifier) (scoped_type_identifier) (generic_type)] @reference.implements)))

(interface_declaration
  name: (_) @reference.source
  (extends_interfaces
    (type_list [(type_identifier) (scoped_type_identifier) (generic_type)] @reference.inherits)))

; Calls: `total()`, and `Cart.of()` or `cart.total()` through their receiver's name, which
; resolves when it is a class (imported, of the package, or qualified).
(method_invocation !object name: (identifier) @reference.call)

(method_invocation object: [(identifier) (field_access)] @reference.call)

(object_creation_expression
  type: [(type_identifier) (scoped_type_identifier) (generic_type)] @reference.call)

; Annotations: `@Entity`, `@RestController`.
(marker_annotation name: (_) @reference.type)

(annotation name: (_) @reference.type)

; Types in declarations, parameters, casts and generics.
(scoped_type_identifier) @reference.type

(type_identifier) @reference.type
