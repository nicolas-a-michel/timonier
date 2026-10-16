; Imports, calls, references and heritage of the TypeScript pack (contract: ADR 0012).
; A node captured by several patterns keeps the first one, and a reference nested in another
; reference is dropped: the outer path wins.

; Imports and re-exports: the module specifier, and the local names an import binds.
(import_statement source: (string (string_fragment) @import.path)) @import

(export_statement source: (string (string_fragment) @import.path)) @import

(import_specifier alias: (identifier) @import.name)

(import_specifier name: (identifier) @import.name !alias)

(namespace_import (identifier) @import.name)

(import_clause (identifier) @import.name)

; Re-exports bind names too: `export { a, b as c } from "./x"`.
(export_statement
  (export_clause (export_specifier alias: (_) @import.name))
  source: (_))

(export_statement
  (export_clause (export_specifier name: (_) @import.name !alias))
  source: (_))

; `import x = require("y")`
(import_require_clause (identifier) @import.name source: (string (string_fragment) @import.path)) @import

; Calls and constructions. JSX elements are in references.tsx.scm: only the tsx grammar has
; them.
(call_expression function: (identifier) @reference.call)

; A member call resolves through its first segment when it is imported (`ns.k()`), never by its
; name alone (ADR 0013).
(call_expression function: (member_expression) @reference.method)

(new_expression constructor: [(identifier) (member_expression)] @reference.call)

; Heritage: `class A extends B implements I`, `interface J extends K`.
(class_declaration
  name: (_) @reference.source
  (class_heritage (implements_clause [(type_identifier) (nested_type_identifier)] @reference.implements)))

(class_declaration
  name: (_) @reference.source
  (class_heritage (extends_clause value: [(identifier) (member_expression)] @reference.inherits)))

(abstract_class_declaration
  name: (_) @reference.source
  (class_heritage (implements_clause [(type_identifier) (nested_type_identifier)] @reference.implements)))

(abstract_class_declaration
  name: (_) @reference.source
  (class_heritage (extends_clause value: [(identifier) (member_expression)] @reference.inherits)))

(interface_declaration
  name: (_) @reference.source
  (extends_type_clause [(type_identifier) (nested_type_identifier)] @reference.inherits))

; Types cited anywhere else.
(nested_type_identifier) @reference.type

(type_identifier) @reference.type
