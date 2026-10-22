; Includes, calls and references of the C pack (contract: ADR 0012). A node captured by
; several patterns keeps the first one, and a reference nested in another reference is
; dropped: the outer path wins.

; `#include "cart.h"` and `#include <sqlite3.h>`: an include binds no name (ADR 0065, 0068).
(preproc_include path: (string_literal (string_content) @import.path)) @import

(preproc_include path: (system_lib_string) @import.path) @import

; Calls: `cart_total(c)`; a call through a pointer of a structure (`c->total(c)`) never by its
; name alone (ADR 0013).
(call_expression function: (identifier) @reference.call)

(call_expression function: (field_expression) @reference.method)

; Types in declarations, parameters and casts.
(type_identifier) @reference.type
