---
name: adr
description: Template and procedure for writing an Architecture Decision Record in docs/adr/. Use whenever a technical decision is not covered by docs/spec.md — a library or tool choice, a file format, a protocol detail, a repository layout, a threshold, the interpretation of an ambiguous point — before or together with the code that relies on it.
---

# Architecture Decision Records

Project rule 3: every technical decision not covered by `docs/spec.md` gets a short ADR in
`docs/adr/NNNN-titre.md`, with context, decision and rejected alternatives.

## When not to write one

- The spec already decides the point, for example in « Décisions techniques ». Cite the spec
  section in the code or in `PLAN.md` instead.
- The decision **changes product behaviour**, or contradicts or completes the spec. Do not
  decide it in an ADR: stop and ask the human (rule « ne rien inventer »). Once the human has
  answered, an ADR may record the answer.
- Purely local naming or refactoring choices.

## Procedure

1. Read `docs/adr/README.md`. If an ADR already covers the subject, update it (while it is
   still `Proposé`) or supersede it.
2. Take the next number (the highest existing number plus one, on four digits). File name:
   `docs/adr/NNNN-kebab-case-title.md`.
3. Copy [template.md](template.md). Write it **in French**: ADRs are read by the human. Keep it
   short, one page at most.
4. Status lifecycle: `Proposé` → `Accepté` once the human has validated the phase or the
   decision → possibly `Remplacé par NNNN`. Never rewrite the decision of an accepted ADR:
   write a new one that supersedes it, and update the old one's status line only.
5. If the decision changes a convention described in a skill, in `CLAUDE.md` or in the hooks,
   update them in the same change.
6. Add the line to the index in `docs/adr/README.md`, and reference the ADR from the
   « Décisions » line of the phase in `PLAN.md`.
7. Any package or crate named in the decision has been verified: it exists, and its current
   version and licence are stated in the ADR with the date of the check.
