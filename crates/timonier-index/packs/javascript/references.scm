; Imports, calls and references of the JavaScript pack (contract: ADR 0012). A node captured by
; several patterns keeps the first one, and a reference nested in another reference is dropped:
; the outer path wins.

; ES modules: the module specifier, and the local names an import binds.
(import_statement source: (string (string_fragment) @import.path)) @import

(export_statement source: (string (string_fragment) @import.path)) @import

(import_specifier alias: (identifier) @import.name)

(import_specifier name: (identifier) @import.name !alias)

(namespace_import (identifier) @import.name)

(import_clause (identifier) @import.name)

(export_statement
  (export_clause (export_specifier alias: (_) @import.name))
  source: (_))

(export_statement
  (export_clause (export_specifier name: (_) @import.name !alias))
  source: (_))

; CommonJS: `const x = require("y")`, `const { a, b } = require("y")`.
(variable_declarator
  name: (identifier) @import.name
  value: (call_expression
    function: (identifier) @_require
    arguments: (arguments (string (string_fragment) @import.path)))
  (#eq? @_require "require")) @import

(variable_declarator
  name: (object_pattern
    [
      (shorthand_property_identifier_pattern) @import.name
      (pair_pattern value: (identifier) @import.name)
    ])
  value: (call_expression
    function: (identifier) @_require
    arguments: (arguments (string (string_fragment) @import.path)))
  (#eq? @_require "require")) @import

; Calls and constructions, JSX elements included.
(call_expression function: (identifier) @reference.call)

; A member call resolves through its first segment when it is imported (`ns.k()`), never by its
; name alone (ADR 0013).
(call_expression function: (member_expression) @reference.method)

(new_expression constructor: [(identifier) (member_expression)] @reference.call)

(jsx_opening_element name: (_) @reference.call)

(jsx_self_closing_element name: (_) @reference.call)

; Heritage: `class A extends B`.
(class_declaration
  name: (_) @reference.source
  (class_heritage [(identifier) (member_expression)] @reference.inherits))
