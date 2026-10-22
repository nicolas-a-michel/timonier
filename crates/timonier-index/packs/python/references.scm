; Imports, calls, references and heritage of the Python pack (contract: ADR 0012). A node
; captured by several patterns keeps the first one, and a reference nested in another
; reference is dropped: the outer path wins.

; `import a.b` binds `a`; `import a.b as c` binds `c`.
(import_statement
  name: (dotted_name . (identifier) @import.name) @import.path) @import

(import_statement
  name: (aliased_import name: (dotted_name) @import.path alias: (identifier) @import.name)) @import

; `from a.b import c, d as e`, and relative ones (`from . import x`, `from ..m import y`).
(import_from_statement module_name: (_) @import.path) @import

(import_from_statement name: (dotted_name (identifier) @import.name))

(import_from_statement name: (aliased_import alias: (identifier) @import.name))

; Calls and constructions: Python makes no difference between them.
(call function: (identifier) @reference.call)

; A member call resolves through its first segment when it is imported (`cart.total()`), never
; by its name alone (ADR 0013).
(call function: (attribute) @reference.method)

; Heritage: `class A(B, m.C)`.
(class_definition
  name: (_) @reference.source
  superclasses: (argument_list [(identifier) (attribute)] @reference.inherits))

; Types in annotations.
(type [(identifier) (attribute)] @reference.type)
