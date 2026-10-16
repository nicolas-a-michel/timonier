; Imports, calls, references, implementations of the Rust pack (contract: ADR 0012).
; A node captured by several patterns keeps the first one, and a reference nested in another
; reference is dropped: the outer path wins.

; Imports. Each path a `use` brings in; the engine prepends the `path` of the enclosing lists.
; A last segment `*` imports the whole module without binding a name.
(use_declaration
  argument: [(identifier) (scoped_identifier) (crate) (self) (super)] @import.path) @import

(use_declaration
  argument: (use_as_clause path: (_) @import.path alias: (_) @import.alias)) @import

(use_declaration
  argument: (use_wildcard) @import.path) @import

(use_declaration
  argument: (scoped_use_list)) @import

(use_list [(identifier) (scoped_identifier) (self) (crate) (super)] @import.path)

(use_list (use_as_clause path: (_) @import.path alias: (_) @import.alias))

(use_list (use_wildcard) @import.path)

; `extern crate foo;`
(extern_crate_declaration name: (_) @import.path) @import

; Calls.
(call_expression
  function: [(identifier) (scoped_identifier)] @reference.call)

(call_expression
  function: (generic_function function: [(identifier) (scoped_identifier)] @reference.call))

; A method on a receiver: its type is unknown here, the LSP resolves it (ADR 0013).
(call_expression
  function: (field_expression field: (_) @reference.method))

(call_expression
  function: (generic_function function: (field_expression field: (_) @reference.method)))

(struct_expression name: (_) @reference.type)

; Implementations: `impl Trait for Type`, from the type defined in this file.
(impl_item
  trait: (_) @reference.implements
  type: [
    (type_identifier) @reference.source
    (generic_type type: (_) @reference.source)
  ])

; Types and paths cited anywhere else.
(scoped_type_identifier) @reference.type

(type_identifier) @reference.type

(scoped_identifier) @reference.type
