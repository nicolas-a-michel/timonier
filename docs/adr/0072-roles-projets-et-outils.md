# 0072 — Les rôles : un dossier de plusieurs projets, le vote des bibliothèques, les outils

- Statut : Proposé
- Date : 2026-10-22
- Phase : 9
- Liens : Q91 ; ADR 0016 (composants), 0017 (rôles) ; tâche PLAN.md 9.7

## Contexte

Sur `GPU-Goblin`, un dossier qui réunit six dépôts git (gg-core, gg-server, gg-worker,
gg-python, gg-web, gg-console), la carte rangeait gg-core, gg-server et gg-worker dans le groupe
« Point d'entrée ». Trois causes : un composant du haut prenait le rôle le plus fréquent de ses
paquets sans compter les utilitaires, donc un seul binaire (gg-client) en faisait un point
d'entrée face à quatre bibliothèques ; `xtask` et le `bin/` d'une copie vendorisée comptaient
comme points d'entrée ; et des regroupements s'appelaient « mod », du nom de leur `mod.rs`.
L'humain a validé les quatre corrections (Q91).

## Décision

- **Rôle `project`** (huitième rôle) : un dossier du premier niveau qui a son propre dépôt (un
  dossier `.git`, ou le fichier `.git` d'un sous-module ou d'un worktree) est un projet. S'il
  contient plus d'un élément, il devient le composant `dir/<dossier>`, à la racine, rangé à part
  (ADR 0016 s'applique à l'intérieur) ; un projet d'un seul paquet est ce paquet. Les deux
  prennent le rôle `project`, et la carte place leur groupe en premier, en turquoise avec un
  bord de 2 px (Q93). Les rôles reprennent à l'intérieur de chaque projet.
- **Les bibliothèques votent** : le rôle d'un composant du haut est le plus fréquent de ses
  paquets, utilitaires compris ; à égalité, le rôle qui dit plus qu'« utilitaire » l'emporte
  (ordre de l'ADR 0017).
- **Les outils de développement** : un paquet dont le chemin passe par `xtask`, `tools`,
  `scripts`, `vendor`, `third_party` ou `third-party` est utilitaire, et les fichiers de ces
  chemins ne donnent aucun signal de rôle (comme les tests).
- **Les noms** : un regroupement dont le fichier central ne dit que « module » (`mod`, `index`,
  `__init__`, `lib`, `main`, `init`) prend le nom de son dossier (`src/` sauté).

## Alternatives écartées

- **Détecter un projet par son manifeste** : un monorepo ordinaire (`frontend/`, `backend/`,
  chacun son `package.json`) aurait perdu ses rôles, qui y sont justes.
- **Des projets à toute profondeur** : seul le premier niveau a été rencontré ; chercher plus
  bas coûterait un test par dossier à chaque parcours.
- **Garder l'exclusion des utilitaires du vote** : c'est ce qui faisait d'une bibliothèque
  partagée un point d'entrée.

## Conséquences

Les clés des composants ne changent pas (`dir/gg-core` existait déjà) : l'historique et la
relecture suivent. Un dépôt dont un sous-module est au premier niveau montre ce sous-module en
projet. Un composant qui mêlait deux projets disparaît au profit des deux projets.
