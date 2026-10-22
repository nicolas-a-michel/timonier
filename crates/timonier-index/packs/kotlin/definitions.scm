; Definitions of the Kotlin pack (contract: ADR 0012). When several patterns capture the same
; node, the first one wins.

(class_declaration "interface" name: (_) @name (class_body) @body) @definition.trait

(class_declaration "interface" name: (_) @name) @definition.trait

(class_declaration name: (_) @name [(class_body) (enum_class_body)] @body) @definition.type

(class_declaration name: (_) @name) @definition.type

(object_declaration name: (_) @name (class_body) @body) @definition.type

(object_declaration name: (_) @name) @definition.type

; A companion object contains its members: `Cart.Companion.create`, or its own name.
(companion_object name: (_) @name) @scope

((companion_object) @scope
  (#set! timonier.segment "Companion"))

(type_alias type: (identifier) @name) @definition.type

(enum_entry (identifier) @name) @definition.constant

; Methods and properties of a class, an object or a companion.
(class_body (function_declaration name: (_) @name (function_body) @body) @definition.method)

(class_body (function_declaration name: (_) @name) @definition.method)

(class_body (property_declaration (variable_declaration (identifier) @name)) @definition.field)

; `class Cart(val owner: String)`: a property declared by the constructor.
(class_parameter ["val" "var"] (identifier) @name) @definition.field

; Functions and properties of a file.
(function_declaration name: (_) @name (function_body) @body) @definition.function

(function_declaration name: (_) @name) @definition.function

(source_file (property_declaration (variable_declaration (identifier) @name)) @definition.constant)
