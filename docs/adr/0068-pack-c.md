# 0068 — Le pack C : CMake partagé avec C++, en-têtes lus comme du C++, une API C sur opentelemetry-cpp

- Statut : Accepté (2026-10-22, Phase 10 validée)
- Date : 2026-10-22
- Phase : 10
- Liens : Q86, Q88, Q89 ; ADR 0012, 0013, 0034, 0050, 0065 (pack C++) ; tâche PLAN.md 10.8

## Contexte

C partage avec C++ ses outils (CMake, CTest, vcpkg, Conan) et ses en-têtes : un `.h` peut être
lu par l'un ou l'autre. OpenTelemetry n'a pas de SDK C ; son SDK C++ est celui que le pack C++
installe déjà. Les en-têtes protègent leurs déclarations par une garde (`#ifndef CART_H`), et
un fichier atteint souvent une déclaration à travers un autre en-tête.

## Décision

- **Pack `c`** : grammaire `tree-sitter-c` 0.24.2 (MIT, vérifiée le 2026-10-22, chargée par
  tree-sitter 0.27), `.c` seulement. Les `.h` restent au pack C++, dont la grammaire lit aussi
  le C : un en-tête de C est nommé `cpp:…`. Définitions : structures, unions et énumérations
  avec un corps, leurs champs et énumérateurs, typedefs (une structure anonyme prend le nom de
  son typedef ; un typedef d'une structure nommée est cette structure), fonctions, prototypes
  et variables de fichier, macros (`#define`).
- **Noms** : `c:<chemin avec extension>#<définition>` (`c:src/cart.c#cart_total`), comme C++.
- **Écosystème** : `cmake`, celui de C++ (ADR 0065) : mêmes paquets, dépendances, inventaire ;
  les règles d'inventaire des deux packs s'additionnent.
- **Gardes d'inclusion** : pour C et C++, les prototypes et variables de fichier se lisent
  aussi sous un `#ifndef` ou un `#if` (version du pack C++ passée à 1.0.1).
- **Résolution** : celle de C++, et un nom défini dans plusieurs fichiers mène à celui que le
  fichier inclut, directement ou à travers d'autres en-têtes, sur quatre niveaux.
- **Détection** (`init`) : un projet CMake est en C quand son `project()` nomme C sans CXX, ou,
  n'en nommant aucun, quand il a des `.c` et aucun fichier C++ à la racine et un dossier
  en dessous ; sinon en C++. Un seul lanceur, CTest, dans les deux cas.
- **SDK** (`init`) : le dossier `timonier/` tient une API C (`timonier_otel.h` :
  `timonier_start_telemetry`, `timonier_span_start`, `timonier_span_set`, `timonier_span_end`,
  `timonier_stop_telemetry`) implémentée en C++ (`timonier_otel.cpp`) sur opentelemetry-cpp
  1.29.0 et son exportateur OTLP/HTTP, et le CMake qui la bâtit (`enable_language(CXX)`) quand
  le SDK est trouvé ; `vcpkg.json` reçoit opentelemetry-cpp comme pour C++. Le test de fumée
  est en C. Ce n'est pas un SDK de plus : chaque appel passe à celui de C++.
- **Trous d'instrumentation** : publique, une fonction qui n'est pas `static` ; `*_test.c` est
  du code de test.

## Alternatives écartées

- **Donner les `.h` au pack C** : la grammaire C lit mal les en-têtes de C++ (classes,
  espaces de noms), alors que celle de C++ lit ceux de C.
- **Écrire un exportateur OTLP en C** : un SDK à maintenir, ce que la spec ne demande pas.
- **Un écosystème à part pour C** : les mêmes `CMakeLists.txt` donneraient deux paquets.

## Conséquences

- Le projet C qui veut ses traces a besoin d'un compilateur C++ pour bâtir l'API.
- Un en-tête de C a un identifiant `cpp:` ; l'outil le montre comme du C++.
