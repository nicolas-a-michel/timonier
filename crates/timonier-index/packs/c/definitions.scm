; Definitions of the C pack (contract: ADR 0012). When several patterns capture the same node,
; the first one wins. A declaration without a body (`struct cart;`) defines nothing.

; `typedef struct { … } item;`: the anonymous structure takes the typedef's name. A typedef of a
; named structure with a body is that structure; one of a type without a body is an alias.
(type_definition
  type: [(struct_specifier !name body: (_) @body) (union_specifier !name body: (_) @body) (enum_specifier !name body: (_) @body)]
  declarator: (type_identifier) @name) @definition.type

(type_definition type: (_ !body) declarator: (type_identifier) @name) @definition.type

(type_definition type: (_ !body) declarator: (pointer_declarator declarator: (type_identifier) @name)) @definition.type

(struct_specifier name: (_) @name body: (_) @body) @definition.type

(union_specifier name: (_) @name body: (_) @body) @definition.type

(enum_specifier name: (_) @name body: (_) @body) @definition.type

(enumerator name: (_) @name) @definition.constant

; Fields of a structure or a union.
(field_declaration_list
  (field_declaration
    declarator: [
      (field_identifier) @name
      (pointer_declarator declarator: (field_identifier) @name)
      (array_declarator declarator: (field_identifier) @name)
    ]) @definition.field)

(function_definition
  declarator: [
    (function_declarator declarator: (identifier) @name)
    (pointer_declarator declarator: (function_declarator declarator: (identifier) @name))
  ]
  body: (_) @body) @definition.function

; Prototypes and variables of a file, inside its include guard too (`#ifndef CART_H`).
(translation_unit
  (declaration
    declarator: [
      (function_declarator declarator: (identifier) @name)
      (pointer_declarator declarator: (function_declarator declarator: (identifier) @name))
    ]) @definition.function)

(preproc_ifdef
  (declaration
    declarator: [
      (function_declarator declarator: (identifier) @name)
      (pointer_declarator declarator: (function_declarator declarator: (identifier) @name))
    ]) @definition.function)

(preproc_if
  (declaration
    declarator: [
      (function_declarator declarator: (identifier) @name)
      (pointer_declarator declarator: (function_declarator declarator: (identifier) @name))
    ]) @definition.function)

(translation_unit
  (declaration declarator: (init_declarator declarator: (identifier) @name)) @definition.constant)

(preproc_ifdef
  (declaration declarator: (init_declarator declarator: (identifier) @name)) @definition.constant)

(preproc_if
  (declaration declarator: (init_declarator declarator: (identifier) @name)) @definition.constant)

; Macros of a file: `#define MAX_ITEMS 16`, `#define SQUARE(x) …`.
(preproc_def name: (_) @name) @definition.constant

(preproc_function_def name: (_) @name) @definition.function
