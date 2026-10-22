; Definitions of the C# pack (contract: ADR 0012). When several patterns capture the same node,
; the first one wins. A namespace is a module: `using` finds a namespace by its declaration
; (ADR 0069).

(namespace_declaration name: (_) @name body: (_) @body) @definition.module

(file_scoped_namespace_declaration name: (_) @name) @definition.module

(class_declaration name: (_) @name body: (_) @body) @definition.type

(struct_declaration name: (_) @name body: (_) @body) @definition.type

(record_declaration name: (_) @name) @definition.type

(enum_declaration name: (_) @name body: (_) @body) @definition.type

(enum_member_declaration name: (_) @name) @definition.constant

(interface_declaration name: (_) @name body: (_) @body) @definition.trait

(delegate_declaration name: (_) @name) @definition.type

(method_declaration name: (_) @name body: (_) @body) @definition.method

(method_declaration name: (_) @name) @definition.method

(constructor_declaration name: (_) @name body: (_) @body) @definition.method

(property_declaration name: (_) @name) @definition.field

(field_declaration (variable_declaration (variable_declarator name: (_) @name))) @definition.field

(event_field_declaration (variable_declaration (variable_declarator name: (_) @name))) @definition.field

; `record Item(string Name, int Price)`: properties declared by the record's parameters.
(record_declaration (parameter_list (parameter name: (_) @name) @definition.field))

; Top-level statements and local functions are not definitions of the file.
