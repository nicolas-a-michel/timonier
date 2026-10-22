; Imports, calls, references and heritage of the Kotlin pack (contract: ADR 0012). A node
; captured by several patterns keeps the first one, and a reference nested in another
; reference is dropped: the outer path wins.

; `import com.shop.Cart` binds `Cart`; `import java.time.Instant as Now` binds `Now`;
; `import com.shop.*` binds no name: the names it brings are looked up in that package.
(import (qualified_identifier) @import.path (identifier) @import.alias) @import

(import (qualified_identifier) @import.path "*" @import.alias) @import

((import (qualified_identifier) @import.path .) @import
  (#not-match? @import "[*]|\\sas\\s"))

; Heritage first, since the types below would take these nodes: a superclass is called
; (`Base()`), an interface is not.
(class_declaration
  name: (_) @reference.source
  (delegation_specifiers
    (delegation_specifier
      (constructor_invocation (user_type . (identifier) @reference.inherits)))))

(class_declaration
  name: (_) @reference.source
  (delegation_specifiers
    (delegation_specifier (user_type . (identifier) @reference.implements))))

(object_declaration
  name: (_) @reference.source
  (delegation_specifiers
    (delegation_specifier (user_type . (identifier) @reference.implements))))

; Calls and constructions: `total()`, `Cart("a")`; `Cart.create()` and `Registry.items`
; through their first name, which resolves when it is a class or an object (ADR 0066).
(call_expression . (identifier) @reference.call)

(navigation_expression . (identifier) @reference.call)

; Types, in declarations, generics and annotations.
(user_type (identifier) @reference.type)
