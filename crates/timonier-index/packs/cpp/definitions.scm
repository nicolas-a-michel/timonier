; Definitions of the C++ pack (contract: ADR 0012). When several patterns capture the same
; node, the first one wins. A forward declaration (`class Cart;`) defines nothing.

(namespace_definition name: (_) @name body: (_) @body) @definition.module

(class_specifier name: (_) @name body: (_) @body) @definition.type

(struct_specifier name: (_) @name body: (_) @body) @definition.type

(union_specifier name: (_) @name body: (_) @body) @definition.type

(enum_specifier name: (_) @name body: (_) @body) @definition.type

(enumerator name: (_) @name) @definition.constant

(alias_declaration name: (_) @name type: (_) @body) @definition.type

(type_definition declarator: (type_identifier) @name) @definition.type

(concept_definition name: (_) @name) @definition.trait

; Methods defined or declared in a class body.
(field_declaration_list
  (function_definition
    declarator: [
      (function_declarator declarator: (_) @name)
      (pointer_declarator declarator: (function_declarator declarator: (_) @name))
      (reference_declarator (function_declarator declarator: (_) @name))
    ]
    body: (_) @body) @definition.method)

(field_declaration_list
  (field_declaration
    declarator: [
      (function_declarator declarator: (_) @name)
      (pointer_declarator declarator: (function_declarator declarator: (_) @name))
      (reference_declarator (function_declarator declarator: (_) @name))
    ]) @definition.method)

(field_declaration_list
  (declaration
    declarator: (function_declarator declarator: (_) @name)) @definition.method)

; Fields.
(field_declaration_list
  (field_declaration
    declarator: [
      (field_identifier) @name
      (pointer_declarator declarator: (field_identifier) @name)
      (reference_declarator (field_identifier) @name)
      (array_declarator declarator: (field_identifier) @name)
    ]) @definition.field)

; A method defined outside its class, `int Cart::total() const { … }`: its scope names the
; container, and the method is ranged under the class when the class is in the same file
; (ADR 0065).
((function_definition
   declarator: [
     (function_declarator declarator: (qualified_identifier scope: (_) @scope.type))
     (pointer_declarator declarator: (function_declarator declarator: (qualified_identifier scope: (_) @scope.type)))
     (reference_declarator (function_declarator declarator: (qualified_identifier scope: (_) @scope.type)))
   ]
   body: (_)) @scope
  (#set! timonier.segment "{scope.type}")
  (#set! timonier.parent "{scope.type}"))

(function_definition
  declarator: [
    (function_declarator declarator: (qualified_identifier name: (_) @name))
    (pointer_declarator declarator: (function_declarator declarator: (qualified_identifier name: (_) @name)))
    (reference_declarator (function_declarator declarator: (qualified_identifier name: (_) @name)))
  ]
  body: (_) @body) @definition.method

(function_definition
  declarator: [
    (function_declarator declarator: (_) @name)
    (pointer_declarator declarator: (function_declarator declarator: (_) @name))
    (reference_declarator (function_declarator declarator: (_) @name))
  ]
  body: (_) @body) @definition.function

; Prototypes and variables of a file or a namespace, inside its include guard too
; (`#ifndef CART_H`).
(translation_unit
  (declaration
    declarator: [
      (function_declarator declarator: (_) @name)
      (pointer_declarator declarator: (function_declarator declarator: (_) @name))
      (reference_declarator (function_declarator declarator: (_) @name))
    ]) @definition.function)

(preproc_ifdef
  (declaration
    declarator: [
      (function_declarator declarator: (_) @name)
      (pointer_declarator declarator: (function_declarator declarator: (_) @name))
      (reference_declarator (function_declarator declarator: (_) @name))
    ]) @definition.function)

(preproc_if
  (declaration
    declarator: [
      (function_declarator declarator: (_) @name)
      (pointer_declarator declarator: (function_declarator declarator: (_) @name))
      (reference_declarator (function_declarator declarator: (_) @name))
    ]) @definition.function)

(declaration_list
  (declaration
    declarator: [
      (function_declarator declarator: (_) @name)
      (pointer_declarator declarator: (function_declarator declarator: (_) @name))
      (reference_declarator (function_declarator declarator: (_) @name))
    ]) @definition.function)

(translation_unit
  (declaration declarator: (init_declarator declarator: (identifier) @name)) @definition.constant)

(preproc_ifdef
  (declaration declarator: (init_declarator declarator: (identifier) @name)) @definition.constant)

(preproc_if
  (declaration declarator: (init_declarator declarator: (identifier) @name)) @definition.constant)

(declaration_list
  (declaration declarator: (init_declarator declarator: (identifier) @name)) @definition.constant)
