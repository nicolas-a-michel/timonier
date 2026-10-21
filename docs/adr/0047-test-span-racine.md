# 0047 — Le run se signale aux tests, et chaque test devient un span racine

- Statut : Accepté (2026-10-21, Phase 8 validée)
- Date : 2026-10-21
- Phase : 8
- Liens : spec « Performances » (branchement : `OTEL_EXPORTER_OTLP_ENDPOINT`,
  `OTEL_RESOURCE_ATTRIBUTES=timonier.run_id=…,timonier.snapshot=…`, chaque test devient un span
  racine) ; ADR 0043 (stockage), 0045 (niveau 1) ; tâches PLAN.md 8.3, 8.4, 8.6, 8.7

## Contexte

Les attributs du run doivent être connus avant que la commande parte : le run doit avoir son ID
et son snapshot au début, pas à la fin. Un span racine par test, lui, ne peut pas venir de
l'extérieur du processus : le contexte de trace d'un test n'existe que dedans, et les runners
diffèrent (cargo-nextest lance un processus par test et lui donne `NEXTEST_TEST_NAME` et
`NEXTEST_BINARY_ID`, vérifié le 2026-10-21 sur nexte.st ; vitest et `node --test` passent
plusieurs tests dans un même processus). Le JUnit XML, lui, nomme chaque test et sa durée.

## Décision

**Le run commence chez le démon.** `timonier test` demande `run/start` (la commande, l'heure) :
le démon crée le run, à la tête du graphe à ce moment, et rend son ID, son snapshot et
l'adresse du récepteur OTLP. À la fin, `run/finish` y ajoute les mesures et les tests. Un run
jamais fini (Ctrl-C, relais tué) reste, sans mesures, et se lit comme interrompu.

**Ce qui est posé dans l'environnement de la commande** :
- `OTEL_EXPORTER_OTLP_ENDPOINT` : l'adresse du récepteur que rend le démon (ADR à venir en 8.4
  pour les ports ; d'ici là, `http://127.0.0.1:4318`, le port HTTP par défaut d'OTLP) ; une
  valeur déjà posée est remplacée, puisque c'est le but du run ;
- `OTEL_RESOURCE_ATTRIBUTES` : `timonier.run_id=R-<n>,timonier.snapshot=<seq>`, ajoutés après
  ce qui y est déjà (une clé déjà là est remplacée).

**Chaque test devient un span racine, que Timonier fait lui-même** à partir du JUnit XML : un
span par cas de test, sans parent, nommé comme le test, avec `timonier.test.name`,
`timonier.test.classname` et `timonier.synthesized = true`, rattaché au run et au test ; sa durée
est celle du JUnit, son début le `timestamp` du cas quand le runner le donne. Les spans du code
disent à quel test ils appartiennent par `timonier.test.name` (et `timonier.test.classname`),
en attribut de ressource ou de span ; au récepteur (8.4), un span sans parent connu d'un test
devient enfant de la racine de ce test, et la racine s'étend à ses spans quand le JUnit ne date
pas le test. Comment chaque langage le pose est dans ses conventions (8.6) : avec cargo-nextest,
le module d'initialisation copie `NEXTEST_TEST_NAME` et `NEXTEST_BINARY_ID` dans la ressource ;
pour vitest et `node --test`, le code de test pose `timonier.test.name` sur ses spans, comme la
convention TypeScript le demande (relecture n° 1 de la Phase 8, I4 : le fichier de mise en place
prévu ici demanderait un troisième fichier et une modification de la configuration du runner,
au-delà de ce que Q72 permet à `init` ; un span TypeScript sans ce nom reste sous la racine de
son run, pas de son test).

## Alternatives écartées

- **Un span racine ouvert par le code de test** : il faudrait toucher chaque test, et ce que le
  runner sait déjà (nom, durée, échec) serait redit.
- **Rattacher les spans aux tests par leurs dates** : deux tests en parallèle se mélangeraient.
- **Garder la valeur d'`OTEL_EXPORTER_OTLP_ENDPOINT` déjà posée** : les spans partiraient
  ailleurs, et le run n'aurait rien.
- **Demander l'ID du run à la fin** (`run/record`) : les attributs du run ne pourraient pas être
  posés avant la commande.

## Conséquences

- Un test a sa racine même sans instrumentation : la timeline montre au moins les tests et
  leurs durées.
- Les noms du JUnit et ceux qu'écrit le langage doivent concorder : pour nextest, le nom du cas
  est `NEXTEST_TEST_NAME` et sa classe `NEXTEST_BINARY_ID`.
- `run/record` (8.2) est remplacé par `run/start` et `run/finish`.
