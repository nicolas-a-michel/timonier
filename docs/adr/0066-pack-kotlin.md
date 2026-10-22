# 0066 — Le pack Kotlin : Gradle partagé avec Java, déclarations de fichier, sous-projet seul

- Statut : Accepté (2026-10-22, Phase 10 validée)
- Date : 2026-10-22
- Phase : 10
- Liens : Q86, Q88, Q90 ; ADR 0012, 0013, 0034, 0050, 0063 (conteneur), 0064 (pack Java,
  écosystème `maven`) ; tâche PLAN.md 10.6

## Contexte

Kotlin se construit avec Gradle et les coordonnées Maven, comme Java, et vit souvent avec lui
dans un même projet. Il diffère par ses déclarations de fichier (une fonction ou une propriété
hors de toute classe, qu'un import nomme : `import com.shop.pricing.total`), ses objets et ses
objets compagnons, ses propriétés déclarées par le constructeur, et sa visibilité publique par
défaut. `gradle init` met le projet dans un sous-dossier `app/`, sans build racine.

## Décision

- **Pack `kotlin`** : grammaire `tree-sitter-kotlin-ng` 1.1.0 (MIT, tree-sitter-grammars,
  vérifiée le 2026-10-22, ABI 14 chargée par tree-sitter 0.27), `.kt` et `.kts` ;
  `tree-sitter-kotlin` 0.3.8 date de 2024. Définitions : classes, interfaces (`trait`), objets,
  alias, entrées d'énumération, méthodes, propriétés du corps et du constructeur (`val`, `var`),
  fonctions et propriétés de fichier. Un objet compagnon est un conteneur : `Companion`, ou son
  nom. `[walk] skip` : `build`, `out`, `.gradle`.
- **Noms** : `kotlin:<paquet>.<Fichier>#<définition>`, depuis la racine `src/<ensemble>/kotlin`,
  comme Java (ADR 0064).
- **Écosystème** : `maven`, celui de Java : mêmes builds, catalogue et settings.
- **Résolution** : un import lie son dernier segment ou son alias (`as`), `*` ne lie rien ; il
  mène à la classe du plus long préfixe, Java ou Kotlin, puis à la déclaration du paquet qui
  porte ce nom (une fonction de fichier, quel que soit le nom du fichier), d'un module ou de
  l'autre. Java et Kotlin se voient dans les deux sens. Un appel `Cart.create()` passe par son
  premier nom, comme en Java. Les imports `*` de Java passent désormais par la même résolution.
- **Détection** (`init`) : un build Gradle qui applique Kotlin (le mot `kotlin` dans les
  settings, le build racine, ceux des projets inclus ou le catalogue) est Kotlin, et non Java ;
  un seul lanceur, donc une seule édition du build.
- **Tests** : les lanceurs Gradle de Java (enveloppe d'abord), rapports par dossier.
- **SDK** (`init`) : les mêmes artefacts que Java, 1.67.0 ; objet `timonier.TimonierOtel` et
  test de fumée JUnit 5 dans `src/test/kotlin/timonier/`. **Sous-projet seul** : un build Gradle
  sans build racine dont les settings incluent un seul projet (`app/`) reçoit le SDK dans le
  build de ce projet, et les fichiers sous son dossier ; un modèle qui cite son propre chemin
  (`code.file.path`) cite celui où il est écrit. Java en profite aussi.
- **Trous d'instrumentation** : publique, une fonction sans `private`, `internal` ni
  `protected`.

## Alternatives écartées

- **Un écosystème `gradle` à part** : Java et Kotlin partagent builds et artefacts ; deux
  écosystèmes doubleraient les paquets d'un projet mixte.
- **Lire la déclaration `package`** : comme pour Java, le chemin depuis la racine de sources est
  connu avant l'extraction ; un fichier Kotlin rangé hors de son paquet est mal nommé.

## Conséquences

- La grammaire 1.1.0 ne lit pas les membres écrits sur la ligne de leur accolade
  (`interface A { fun f() }`) : le fichier est marqué en erreur, le reste est lu.
- Un projet qui mêle Java et Kotlin est détecté Kotlin : ses tests passent par Gradle, et le SDK
  et les fichiers de Kotlin sont écrits.
