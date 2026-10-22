; Definitions of the Go pack (contract: ADR 0012). When several patterns capture the same node,
; the first one wins.

; A method is declared beside its type: its receiver names the container, and the method is
; ranged under the type when the type is defined in the same file (ADR 0063).
((method_declaration
   receiver: (parameter_list
     (parameter_declaration
       type: [
         (type_identifier) @scope.type
         (pointer_type (type_identifier) @scope.type)
         (generic_type type: (type_identifier) @scope.type)
         (pointer_type (generic_type type: (type_identifier) @scope.type))
       ]))) @scope
  (#set! timonier.segment "{scope.type}")
  (#set! timonier.parent "{scope.type}"))

(method_declaration name: (_) @name body: (_) @body) @definition.method

(function_declaration name: (_) @name body: (_) @body) @definition.function

(function_declaration name: (_) @name !body) @definition.function

; Interfaces and their methods.
(type_spec name: (_) @name type: (interface_type) @body) @definition.trait

(interface_type (method_elem name: (_) @name) @definition.method)

(type_spec name: (_) @name type: (_) @body) @definition.type

(type_alias name: (_) @name type: (_) @body) @definition.type

; Fields of a struct type.
(field_declaration name: (_) @name) @definition.field

; Package-level constants and variables.
(source_file (const_declaration (const_spec name: (identifier) @name) @definition.constant))

(source_file (var_declaration (var_spec name: (identifier) @name) @definition.constant))

(source_file
  (var_declaration (var_spec_list (var_spec name: (identifier) @name) @definition.constant)))
