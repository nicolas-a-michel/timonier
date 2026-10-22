; Definitions of the Java pack (contract: ADR 0012). When several patterns capture the same
; node, the first one wins. Every method lives in a type body, so it is a method; overloads
; share a path and the first with a body is kept.

(class_declaration name: (_) @name body: (_) @body) @definition.type

(record_declaration name: (_) @name body: (_) @body) @definition.type

(enum_declaration name: (_) @name body: (_) @body) @definition.type

(annotation_type_declaration name: (_) @name body: (_) @body) @definition.type

(interface_declaration name: (_) @name body: (_) @body) @definition.trait

(method_declaration name: (_) @name body: (_) @body) @definition.method

(method_declaration name: (_) @name !body) @definition.method

(constructor_declaration name: (_) @name body: (_) @body) @definition.method

(field_declaration declarator: (variable_declarator name: (_) @name)) @definition.field

(constant_declaration declarator: (variable_declarator name: (_) @name)) @definition.constant

(enum_constant name: (_) @name) @definition.constant
