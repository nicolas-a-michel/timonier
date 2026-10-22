; Includes, calls, references and heritage of the C++ pack (contract: ADR 0012). A node
; captured by several patterns keeps the first one, and a reference nested in another
; reference is dropped: the outer path wins.

; `#include "cart.hpp"` and `#include <fmt/core.h>`: an include binds no name (ADR 0065).
(preproc_include path: (string_literal (string_content) @import.path)) @import

(preproc_include path: (system_lib_string) @import.path) @import

; Heritage first, since the types below would take these nodes.
(class_specifier
  name: (_) @reference.source
  (base_class_clause [(type_identifier) (qualified_identifier) (template_type)] @reference.inherits))

(struct_specifier
  name: (_) @reference.source
  (base_class_clause [(type_identifier) (qualified_identifier) (template_type)] @reference.inherits))

; Calls: `total()`, `shop::price()`, `Cart::make()`, `make<int>()`; a member call
; (`cart.total()`, `p->total()`) never by its name alone (ADR 0013).
(call_expression function: [(identifier) (qualified_identifier) (template_function)] @reference.call)

(call_expression function: (field_expression) @reference.method)

(new_expression type: (_) @reference.call)

; Types in declarations, parameters, templates and casts.
(qualified_identifier) @reference.type

(template_type) @reference.type

(type_identifier) @reference.type
