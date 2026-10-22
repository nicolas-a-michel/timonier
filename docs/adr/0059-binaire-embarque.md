# 0059 — Le programme `timonier` livré dans l'extension, et dans le PATH

- Statut : Accepté (2026-10-22, Phase 9 validée)
- Date : 2026-10-21
- Phase : 9
- Liens : Q58, Q59, Q12 ; ADR 0019 (lancement du démon), 0033 (init), 0057 (dossier de données) ;
  tâche PLAN.md 9.7

## Contexte

Q58 : un `.vsix` par plateforme (linux-x64, linux-arm64, darwin-x64, darwin-arm64 ; win32-x64 et
win32-arm64 depuis Q87) embarque le
programme construit par la CI, et rien n'est à installer à part. L'extension peut lancer ce
programme par son chemin. Les agents, eux, l'appellent par son nom : `.mcp.json`,
`.vscode/mcp.json` et les hooks écrits par `init` disent `timonier`, et ces fichiers sont
versionnés et partagés, donc sans chemin propre à une machine. Le dossier de l'extension change à
chaque mise à jour (`…/nicolasmichel.timonier-0.1.0-linux-x64/`). Q58 laissait à un ADR le choix
entre un chemin exposé et une commande qui l'installe dans le PATH.

## Décision

- **Dans le `.vsix`** : `bin/timonier`, construit en release par la CI de publication sur des
  runners natifs : `ubuntu-22.04` (x64) et `ubuntu-22.04-arm` (arm64), pour la glibc 2.35 de 22.04
  plutôt que celle de 24.04 ; `macos-15-intel` et `macos-15` ; `windows-2022` et `windows-11-arm`
  (labels vérifiés le 2026-10-21 dans la documentation de GitHub). Un paquet par cible
  (`vsce package --target`) ; `bin/timonier.exe` sous Windows.
- **Le programme que l'extension lance** : `TIMONIER_DAEMON_PATH`, puis le réglage
  `timonier.daemon.path`, puis le programme embarqué, puis `timonier` du PATH.
- **Dans le PATH, pour les agents** : la commande « Timonier : Installer la ligne de commande »
  copie le programme embarqué dans `<données>/bin/timonier` (le dossier de l'ADR 0057), puis fait
  pointer `~/.local/bin/timonier` et `~/.local/bin/tim` vers cette copie. Un `timonier` déjà dans
  le PATH qui n'est pas cette copie (par exemple `cargo install`) n'est pas touché ; un lien de
  `~/.local/bin` qui pointe ailleurs non plus. « Configurer » du pont l'installe d'abord quand
  `timonier` manque au PATH : configurer pour des agents qui ne trouveraient pas le programme ne
  servirait à rien.
- **Mise à jour** : à chaque activation, une copie installée par l'extension et différente du
  programme embarqué est remplacée (écrite à côté, puis renommée), sans toucher aux liens.
- Si `~/.local/bin` n'est pas dans le PATH, l'extension le dit.
- **Windows** (Q87, ADR 0060) : pas de lien symbolique sans le mode développeur. La commande
  copie `timonier.exe` et `tim.exe` dans `%LOCALAPPDATA%\timonier\bin` et ajoute ce dossier au PATH
  de l'utilisateur (`[Environment]::SetEnvironmentVariable(…, 'User')` par PowerShell), que les
  processus lancés ensuite voient ; l'extension dit de redémarrer VSCode et les agents.

## Alternatives écartées

- **Un chemin absolu dans `.mcp.json` et les hooks** : fichiers versionnés, partagés entre
  machines.
- **Un lien vers le dossier de l'extension** : il casse à chaque mise à jour de l'extension.
- **Modifier le profil du shell** : plus intrusif qu'un lien dans `~/.local/bin`, que les
  distributions courantes mettent déjà dans le PATH.
- **Installer sans le demander** : écrire dans `~/.local/bin` est un geste du développeur ; la
  commande, ou « Configurer », en est l'accord.

## Conséquences

- Un développeur qui a son propre `timonier` (`cargo install`) garde le sien pour les agents ;
  l'extension lance le programme embarqué. Deux versions peuvent alors se croiser : le démon
  refuse déjà un client d'une autre version et le dit (ADR 0019).
- Les six paquets se construisent sur six runners en parallèle.
- Un `~/.local/bin` créé après l'ouverture de session n'est dans le PATH qu'à la session
  suivante sur Ubuntu : l'extension le dit.
