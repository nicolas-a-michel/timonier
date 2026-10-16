# 0003 — Découpage des crates, sens des dépendances et alias `tim`

- Statut : Accepté (2026-10-14, validation de la phase par l'humain)
- Date : 2026-10-14
- Phase : 1
- Liens : prompt de démarrage (Phase 1), skill `rust-daemon`, PLAN.md tâches 1.3 à 1.5

## Contexte

Le prompt fixe sept crates (`timonier-core`, `-store`, `-index`, `-daemon`, `-otlp`, `-mcp`,
`-cli`), un binaire `timonier` et un alias `tim`, sans dire comment elles dépendent les unes
des autres, ni où vivent les types du protocole partagés par le démon, le proxy MCP et la CLI.

## Décision

**Dépendances**, toutes orientées vers `timonier-core`, sans cycle :

| Crate | Dépend de |
| ----- | --------- |
| `timonier-core` | aucune crate interne |
| `timonier-store`, `timonier-index`, `timonier-otlp`, `timonier-mcp` | `timonier-core` |
| `timonier-daemon` | `timonier-core`, `-store`, `-index`, `-otlp` |
| `timonier-cli` | `timonier-core`, `-daemon`, `-mcp` |

Les dépendances internes sont déclarées dès la Phase 1, même si elles ne servent pas encore.

**Types du protocole** : ils vivent dans `timonier-core`, sous forme de types seulement, sans
I/O. `timonier-mcp` parle au démon par le socket et ne dépend pas de `timonier-daemon`. C'est
ce qui le garde mince.

**Alias `tim`** : un même programme, deux cibles `[[bin]]` (`timonier` et `tim`) dans
`timonier-cli`, qui appellent toutes deux `timonier_cli::run()`. `--version` affiche
`timonier <version>` sous les deux noms ; l'aide reprend le nom invoqué.

**Lints** de workspace :

- `unsafe_code = "forbid"` ;
- clippy `unwrap_used`, `dbg_macro` et `todo` en avertissement, transformés en erreurs par
  `-D warnings` ;
- `unwrap`/`expect` permis dans les tests (`clippy.toml`).

**Version** : une seule version pour tout le workspace (`version.workspace = true`), exposée
par `timonier_core::VERSION`. `publish = false` tant que la réservation des noms sur crates.io
n'est pas décidée.

## Alternatives écartées

- **Une crate `timonier-protocol` dédiée** : plus propre à terme, mais elle sort de la liste
  du prompt. À reconsidérer si les types du protocole grossissent.
- **`timonier-mcp` dépendant de `timonier-daemon`** : le proxy embarquerait toute la logique,
  contrairement à la spec (« mince proxy »).
- **Un lien symbolique ou une copie du binaire pour `tim`** : non portable, et non couvert
  par `cargo install`.

## Conséquences

- La CLI embarque le démon et le serveur MCP dans un seul exécutable (`timonier daemon …`,
  `timonier mcp`, à venir).
- Une crate qui a besoin d'une autre hors de ce tableau demande une nouvelle ADR.
