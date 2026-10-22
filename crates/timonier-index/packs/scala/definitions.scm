; Definitions of the Scala pack (contract: ADR 0012). When several patterns capture the same
; node, the first one wins.

(class_definition name: (_) @name body: (_) @body) @definition.type

(class_definition name: (_) @name) @definition.type

(object_definition name: (_) @name body: (_) @body) @definition.type

(object_definition name: (_) @name) @definition.type

(trait_definition name: (_) @name body: (_) @body) @definition.trait

(trait_definition name: (_) @name) @definition.trait

(enum_definition name: (_) @name body: (_) @body) @definition.type

(simple_enum_case name: (_) @name) @definition.constant

(full_enum_case name: (_) @name) @definition.constant

(type_definition name: (_) @name) @definition.type

; Members of a class, an object or a trait.
(template_body (function_definition name: (_) @name body: (_) @body) @definition.method)

(template_body (function_declaration name: (_) @name) @definition.method)

(template_body (val_definition pattern: (identifier) @name) @definition.field)

(template_body (var_definition pattern: (identifier) @name) @definition.field)

; `class Cart(val owner: String)`: a field declared by the constructor.
(class_parameter name: (_) @name) @definition.field

; Definitions of a file (Scala 3) or of a package.
(function_definition name: (_) @name body: (_) @body) @definition.function

(compilation_unit (val_definition pattern: (identifier) @name) @definition.constant)
