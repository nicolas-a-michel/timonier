; Definitions of the TypeScript pack (contract: ADR 0012). When several patterns capture the
; same node, the first one wins: arrow functions bound to a constant are functions.

(class_declaration name: (_) @name body: (_) @body) @definition.type

(abstract_class_declaration name: (_) @name body: (_) @body) @definition.type

(interface_declaration name: (_) @name body: (_) @body) @definition.trait

(type_alias_declaration name: (_) @name value: (_) @body) @definition.type

(enum_declaration name: (_) @name body: (_) @body) @definition.type

(internal_module name: (_) @name body: (_) @body) @definition.module

(function_declaration name: (_) @name body: (_) @body) @definition.function

(generator_function_declaration name: (_) @name body: (_) @body) @definition.function

; Overload signatures and `declare function`. An implementation with a body, if any, wins.
(function_signature name: (_) @name) @definition.function

(method_definition name: (_) @name body: (_) @body) @definition.method

(abstract_method_signature name: (_) @name) @definition.method

(public_field_definition name: (_) @name) @definition.field

; Members of interfaces and of named object types. Inline object types (`{ a: string }` as a
; parameter type) are not definitions.
(interface_body (method_signature name: (_) @name) @definition.method)

(interface_body (property_signature name: (_) @name) @definition.field)

(type_alias_declaration
  value: (object_type (method_signature name: (_) @name) @definition.method))

(type_alias_declaration
  value: (object_type (property_signature name: (_) @name) @definition.field))

; Module-level constants. A function bound to a constant is a function.
(program
  (lexical_declaration
    (variable_declarator
      name: (identifier) @name
      value: [
        (arrow_function body: (_) @body)
        (function_expression body: (_) @body)
      ]) @definition.function))

(program
  (export_statement
    declaration: (lexical_declaration
      (variable_declarator
        name: (identifier) @name
        value: [
          (arrow_function body: (_) @body)
          (function_expression body: (_) @body)
        ]) @definition.function)))

(program
  (lexical_declaration
    "const"
    (variable_declarator name: (identifier) @name value: (_) @body) @definition.constant))

(program
  (export_statement
    declaration: (lexical_declaration
      "const"
      (variable_declarator name: (identifier) @name value: (_) @body) @definition.constant)))
