; Definitions of the JavaScript pack (contract: ADR 0012). When several patterns capture the
; same node, the first one wins: arrow functions bound to a constant are functions.

(class_declaration name: (_) @name body: (_) @body) @definition.type

(function_declaration name: (_) @name body: (_) @body) @definition.function

(generator_function_declaration name: (_) @name body: (_) @body) @definition.function

(method_definition name: (_) @name body: (_) @body) @definition.method

(field_definition property: (_) @name) @definition.field

; Module-level functions and constants: `const`, and `let` or `var` bound to a function.
(program
  [
    (lexical_declaration
      (variable_declarator
        name: (identifier) @name
        value: [
          (arrow_function body: (_) @body)
          (function_expression body: (_) @body)
        ]) @definition.function)
    (variable_declaration
      (variable_declarator
        name: (identifier) @name
        value: [
          (arrow_function body: (_) @body)
          (function_expression body: (_) @body)
        ]) @definition.function)
  ])

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
