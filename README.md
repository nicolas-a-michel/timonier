# Timonier

Timonier permet à un développeur de piloter des agents IA sans lire tout le code qu'ils
produisent. Le timonier tient la barre et garde le cap pendant que l'équipage, ici les agents,
fait avancer le navire.

Il répond en permanence à quatre questions :

- **ce que je veux** : l'architecture voulue, dessinée librement sur un tableau blanc ;
- **ce qui existe** : l'architecture réelle, les technologies, la santé du code ;
- **comment ça se comporte** : temps d'exécution et ressources, mesurés pendant les tests ;
- **ce qui se passe** : le plan, la todo en direct, ce que l'agent a fait et où regarder.

Un démon Rust local, un par projet, tient un graphe unifié du code, construit à partir de
tree-sitter et du LSP. L'extension VSCode l'affiche, et les agents le lisent et l'écrivent
par MCP.

## État

**Squelette (0.1.0).** Le dépôt contient le workspace Rust, la webview et l'extension VSCode,
qui ouvre un panneau vide. La spécification est dans [docs/spec.md](docs/spec.md), la feuille
de route dans [PLAN.md](PLAN.md), et les décisions techniques dans [docs/adr/](docs/adr/README.md).

## Construire et tester

Prérequis : Rust stable (`rustup`), Node.js 24 ou plus récent.

```sh
# Rust
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# Webview et extension
npm ci
npm run build              # webview puis extension
npm test                   # tests unitaires (webview, extension, hooks)
npm run test:integration   # tests dans un vrai VSCode (télécharge VSCode la première fois)
npm run lint
npm run typecheck
npm run package            # produit editors/vscode/timonier.vsix
```

Pour essayer l'extension, ouvrez le dépôt dans VSCode et lancez « Run Extension » (F5). Dans
la fenêtre de développement qui s'ouvre, cliquez sur l'icône Timonier (une barre à roue) dans
la barre d'activité, puis sur **Ouvrir la carte**. La commande **Timonier : Ouvrir la carte**
de la palette fait la même chose.

Une fois le binaire construit, la CLI s'appelle `timonier` ou `tim` :

```sh
cargo run --bin tim -- --version
```

## Licence

[MIT](LICENSE) © 2026 Nicolas Michel.
