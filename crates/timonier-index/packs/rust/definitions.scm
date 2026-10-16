; Definitions of the Rust pack (contract: ADR 0012). When several patterns capture the same
; node, the first one wins: methods come before functions.

; Methods: functions in an impl or a trait.
(impl_item
  body: (declaration_list
    (function_item name: (_) @name body: (_) @body) @definition.method))

(trait_item
  body: (declaration_list
    (function_item name: (_) @name body: (_) @body) @definition.method))

(trait_item
  body: (declaration_list
    (function_signature_item name: (_) @name) @definition.method))

; Functions, including those of `extern` blocks.
(function_item name: (_) @name body: (_) @body) @definition.function

(function_signature_item name: (_) @name) @definition.function

; Types.
(struct_item name: (_) @name body: (_) @body) @definition.type

(struct_item name: (_) @name !body) @definition.type

(enum_item name: (_) @name body: (_) @body) @definition.type

(union_item name: (_) @name body: (_) @body) @definition.type

(type_item name: (_) @name type: (_) @body) @definition.type

(associated_type name: (_) @name) @definition.type

(trait_item name: (_) @name body: (_) @body) @definition.trait

; Inline modules. `mod foo;` is not a definition: the file `foo.rs` is the module.
(mod_item name: (_) @name body: (_) @body) @definition.module

; Constants and statics.
(const_item name: (_) @name value: (_) @body) @definition.constant

(const_item name: (_) @name !value) @definition.constant

(static_item name: (_) @name value: (_) @body) @definition.constant

; Named fields of structs, unions and enum variants.
(field_declaration name: (_) @name) @definition.field

; An impl names what it contains. A trait impl is `<Type as Trait>` and its methods are
; ranged under the type, when the type is defined in the same file.
((impl_item
   trait: (_) @scope.trait
   type: [
     (type_identifier) @scope.type
     (generic_type type: (_) @scope.type)
     (scoped_type_identifier) @scope.type
     (reference_type type: (_) @scope.type)
   ]
   body: (_)) @scope
  (#set! timonier.segment "<{scope.type} as {scope.trait}>")
  (#set! timonier.parent "{scope.type}"))

((impl_item
   !trait
   type: [
     (type_identifier) @scope.type
     (generic_type type: (_) @scope.type)
     (scoped_type_identifier) @scope.type
     (reference_type type: (_) @scope.type)
   ]
   body: (_)) @scope
  (#set! timonier.segment "{scope.type}")
  (#set! timonier.parent "{scope.type}"))
