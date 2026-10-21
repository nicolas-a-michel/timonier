# 0057 — Ce qu'un dépôt cloné et un processus local peuvent faire faire à Timonier

- Statut : Proposé
- Date : 2026-10-21
- Phase : 9
- Liens : audit de sécurité du 2026-10-21 (rapport de l'humain, hors du dépôt) ; Q84 ; ADR 0019
  (socket), 0039 (tableau blanc), 0045, 0047, 0048 (runs et récepteur), 0050 (fumée)

## Contexte

L'audit ne trouve rien de critique. Il relève deux points moyens : les commandes de
`.timonier/tests.json`, fichier versionné, lancées par `sh -c`, et le récepteur OTLP qui prend
les spans de tout processus local. Il relève aussi des points faibles : un tableau blanc sans
borne sur l'encre totale, un chemin du relais LSP non confiné, une vue qui ouvre des millions de
boîtes, l'arrêt du démon sans poignée de main. Le modèle de confiance reste celui de
l'utilisateur local : la socket est dans un répertoire 0700 (ADR 0019).

## Décision

- **Commandes du dépôt (Q84)** : une commande de `tests.json` (runner ou fumée) ne part que si
  ce poste l'a approuvée. L'empreinte BLAKE3 du chemin du projet et de la commande est gardée
  hors du dépôt, dans `$TIMONIER_DATA_DIR`, sinon `$XDG_DATA_HOME/timonier`, sinon
  `~/.local/share/timonier`, fichier `approved-commands.json`. `init` approuve ce qu'il
  écrit ; sinon `timonier test --approve` ou `timonier doctor --approve`, après lecture.
  `timonier test` refusé sort en 2, à part des échecs des tests. Une commande donnée après `--`
  est celle du développeur : elle ne demande rien.
- **Récepteur OTLP** : `run/start` tire un jeton de 16 octets (`/dev/urandom`), que `timonier
  test` donne aux tests dans `OTEL_EXPORTER_OTLP_HEADERS` (`x-timonier-token=…`, après les
  en-têtes déjà posés) ; les SDK l'envoient en métadonnée gRPC comme en en-tête HTTP (vérifié dans
  `opentelemetry-otlp` 0.33.1). Le démon ne prend les spans et métriques d'un run qu'avec son
  jeton, gardé en mémoire : un démon relancé pendant un run perd ce run.
- **Tableau blanc** : au plus 200 000 points d'encre en tout, et un fichier de plus de 16 Mio
  refusé sans être lu, partout où le démon le lit.
- **Relais LSP** : un chemin hors de la racine est refusé (`resolveInside`), comme à l'ouverture
  du code.
- **Vues** : une souscription ouvre au plus 10 000 boîtes.
- **Gardés tels quels, et dits** : `daemon/shutdown` sans poignée de main (le même utilisateur,
  pour arrêter un démon d'une autre version, ADR 0019) ; le texte du dépôt que les outils
  rendent à l'agent (conventions, tableau blanc, noms du code) peut porter des instructions,
  sans franchir de frontière : l'agent lit déjà le dépôt ; la télémétrie de Timonier part vers
  `OTEL_EXPORTER_OTLP_ENDPOINT` quand l'environnement le pose (OpenTelemetry standard,
  `OTEL_SDK_DISABLED=true` la coupe) ; le hook qui protège `intent.json` reste un garde-fou
  d'appoint, la règle `deny` et le démon, seul écrivain, sont les barrières.

## Alternatives écartées

- **L'empreinte de `tests.json` dans `init.json`** : versionnée elle aussi, un dépôt hostile
  change les deux (Q84).
- **Un secret dans l'URL de l'endpoint** : le gRPC d'OTLP a ses propres chemins ; un en-tête vaut
  pour les deux protocoles.
- **Une limite de débit du récepteur** : le jeton suffit à écarter les autres émetteurs ; un
  test bavard reste borné par les 200 000 spans d'un run (ADR 0043).

## Conséquences

- Sur un poste neuf, ou après un `git pull` qui change `tests.json`, la première commande
  demande `--approve`.
- Un SDK qui n'honore pas `OTEL_EXPORTER_OTLP_HEADERS` ne peut plus envoyer ses spans : les
  conventions des packs le disent.
