# 0017 — Rôles architecturaux et dépendances externes

- Statut : Accepté (2026-10-16, validation de la phase par l'humain)
- Date : 2026-10-16
- Phase : 3
- Liens : spec « Composants » ; Q6 ; ADR 0008 (complétée), 0012, 0016 ; tâche PLAN.md 3.9

## Contexte

La spec donne à chaque composant un rôle architectural : serveur ou point d'entrée, modèle de
données, persistance, interface, tâche de fond, groupe de bibliothèques externes, utilitaire.
Ce rôle est déduit des frameworks et API utilisés, puis confirmé par le LLM. L'humain a choisi
(Q6) des heuristiques par défaut, que l'agent pourra préciser par MCP en Phase 6. Le modèle de
l'ADR 0008 n'a pas d'attribut de rôle : il faut l'ajouter. La vue d'ensemble montre surtout
des paquets (ADR 0016), qui doivent donc porter un rôle eux aussi.

## Décision

**Modèle** : un attribut facultatif `role` sur les nœuds, à valeur dans `entry_point`,
`data_model`, `persistence`, `interface`, `background_task`, `external_libraries`, `utility`.
- Il est renseigné sur les paquets et les composants, absent ailleurs.
- Il fait partie de la version d'un nœud : un rôle qui change est une modification dans le
  diff.
- Migration 0003 : une colonne `role` dans `node`.
- Cette ADR complète l'ADR 0008 sans la modifier.

**Signaux**, tous décrits par les packs (ADR 0012) :
- une bibliothèque déclarée par le paquet (`[[roles]] libraries`, par exemple `rusqlite` pour
  la persistance) compte une fois, pour la confiance de la règle ;
- un chemin importé par un fichier (`[[roles]] paths`, par exemple `vscode` pour une
  interface) compte une fois par paquet, pour la confiance de la règle ;
- un signal du code (`roles.scm`, par exemple une fonction `main` ou un `derive(Serialize)`)
  compte pour la part des fichiers qui l'ont.

Les fichiers de test, d'exemple et de bench n'en donnent pas, car ils ne disent rien de
l'architecture : un segment `tests`, `test`, `__tests__`, `examples` ou `benches` dans le
chemin, ou un nom en `.test.` ou `.spec.`.

**Choix du rôle** : le rôle au meilleur score, s'il atteint 0,5 ; sinon `utility`.
- En cas d'égalité, l'ordre est : point d'entrée, persistance, interface, modèle de données,
  tâche de fond.
- Un composant de dépendances externes a toujours le rôle `external_libraries`.

**Catégories des dépendances externes** : celles de la spec (réseau, sérialisation, base de
données, async), plus `cli`, `observability`, `parsing` et `ui`, quand elles aident à lire la
carte. Elles vivent dans les packs. Une dépendance sans catégorie va dans `other`.

**Confirmation** : en Phase 6, l'agent pourra renommer un composant et corriger un rôle par un
outil MCP structuré. Sa réponse ne sera pas écrasée par l'heuristique suivante ; ce sera à
préciser alors.

## Alternatives écartées

- **Le rôle seulement sur les composants** : sur un petit dépôt, la vue d'ensemble n'est faite
  que de paquets, qui resteraient sans rôle.
- **Compter chaque occurrence d'un signal du code** : un paquet d'analyse qui sérialise deux
  structures deviendrait un « modèle de données ». La part des fichiers mesure mieux ce que le
  paquet est.
- **Un rôle par défaut vide** : la spec veut un rôle pour chaque composant ; `utility` est le
  sien pour ce qui n'a pas de signal.

## Conséquences

- Les rôles attendus de Timonier ne peuvent pas tous être vrais en Phase 3 : le démon n'a pas
  encore de serveur JSON-RPC (Phase 4), ni le serveur MCP de SDK (Phase 6). Le test de 3.9
  vérifie ce que le code permet aujourd'hui, et sera complété avec ces phases.
- L'attribut `role` déborde de l'indexation : les rôles voulus du tableau blanc (Phase 7) sont
  du texte libre, et seront comparés à ces valeurs.
