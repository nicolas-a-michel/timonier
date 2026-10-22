; Definitions of the Python pack (contract: ADR 0012). When several patterns capture the same
; node, the first one wins: a function inside a class body is a method.

(class_definition name: (_) @name body: (_) @body) @definition.type

(class_definition
  body: (block
    [
      (function_definition name: (_) @name body: (_) @body) @definition.method
      (decorated_definition
        definition: (function_definition name: (_) @name body: (_) @body) @definition.method)
    ]))

(function_definition name: (_) @name body: (_) @body) @definition.function

; Fields declared in a class body: `price: int = 0`, `price = 0`.
(class_definition
  body: (block
    (expression_statement
      (assignment left: (identifier) @name) @definition.field)))

; Module-level constants and values: `TAX = 0.2`, `app = Flask(__name__)`.
(module
  (expression_statement
    (assignment left: (identifier) @name right: (_) @body) @definition.constant))
