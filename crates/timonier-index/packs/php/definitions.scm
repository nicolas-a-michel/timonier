; Definitions of the PHP pack (contract: ADR 0012). When several patterns capture the same node,
; the first one wins. A file's namespace comes from Composer's PSR-4 map, not from these
; queries (ADR 0070).

(class_declaration name: (_) @name body: (_) @body) @definition.type

(enum_declaration name: (_) @name body: (_) @body) @definition.type

(enum_case name: (_) @name) @definition.constant

(interface_declaration name: (_) @name body: (_) @body) @definition.trait

(trait_declaration name: (_) @name body: (_) @body) @definition.trait

(method_declaration name: (_) @name body: (_) @body) @definition.method

(method_declaration name: (_) @name) @definition.method

(property_declaration (property_element name: (variable_name (name) @name))) @definition.field

; `public function __construct(private int $price)`: a property declared by the constructor.
(property_promotion_parameter name: (variable_name (name) @name)) @definition.field

(const_declaration (const_element (name) @name)) @definition.constant

(function_definition name: (_) @name body: (_) @body) @definition.function
