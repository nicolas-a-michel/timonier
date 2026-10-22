; Role signals read in PHP code (contract: ADR 0012). Libraries and used namespaces are in
; pack.toml.

; Routes and controllers declared by attributes (Symfony, Laravel's routing attributes).
((attribute (name) @role.entry_point)
  (#any-of? @role.entry_point "Route" "AsController" "Get" "Post"))

; Entities and repositories (Doctrine).
((attribute (name) @role.data_model)
  (#any-of? @role.data_model "Entity" "Table" "Embeddable"))

((attribute (name) @role.persistence)
  (#any-of? @role.persistence "Repository" "AsRepository"))

; Commands and messages handled in the background (Symfony Messenger, console).
((attribute (name) @role.background_task)
  (#any-of? @role.background_task "AsMessageHandler" "AsCronTask" "AsPeriodicTask"))
