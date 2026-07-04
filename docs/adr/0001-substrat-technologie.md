# ADR 0001 — Substrat-Technologie: Sprache, Wahrheit, Beschreibungs-Pflicht, Austauschbarkeit

- **Status:** Akzeptiert
- **Datum:** 2026-07-04
- **Branch:** `scheme-aufbau`
- **Kontext-Dokumente:** Gesetzbuch `semantics/scheme.md`; Implementierungsplan
  `~/.claude/plans/es-soll-eine-neue-sleepy-quail.md`; Geschwister-Substrat
  `../lakearch/` (Gegenpol, append-only/inhaltsadressiert).

> **Reversibilitäts-Leitsatz.** Anders als bei lakearch gibt es hier **keine**
> „irreversibelste" Entscheidung: weil die Wahrheit ein gewöhnlicher Dateisystem-
> baum ist (§5) und die Identität ein **Pfad** (nicht ein Inhalts-Hash), re-hasht
> kein Encoder-Wechsel je „das Universum". Die tragende, teuerste Entscheidung ist
> **die Sprache** (Geschwister-Symmetrie zu lakearch); alles Übrige (Manifest-Format,
> Index-Engine, Wire-Protokoll) ist **billig revidierbar**, weil der Baum die
> alleinige Wahrheit ist und jedes Derivat neu gebaut werden kann.

---

## Entscheidung 1 — Sprache = **Rust**  ·  Reversibilität: schwer (Geschwister-Symmetrie)

- **Wahl:** Rust-Workspace mit denselben drei Crates wie lakearch — `scheme-core`
  (Substrat), `scheme-ffi` (C-ABI für in-Prozess-Embedding), `schemed` (gRPC-Daemon)
  — Edition 2021, MSRV 1.96, `MIT OR Apache-2.0`.
- **Begründung:** scheme agiert **auf derselben Ebene wie lakearch und ist mit ihr
  austauschbar** (§11). Die Anbindung an den Agenten (aigentic) läuft bei lakearch
  über die C-ABI (`lakegrave`, cgo). Damit scheme an **jeder** Ebene ein echtes
  Geschwister ist (core/ffi/daemon) und über denselben Embedding-Weg eingebunden
  wird, ist Rust die kohärente Wahl.
- **Flip-Bedingung:** Fiele das Embedding aus dem Scope (reine Daemon- oder reine
  in-Go-Topologie), wäre ein pure-Go-Substrat tragfähig und deutlich leichter. Diese
  Bedingung tritt nach Nutzerentscheidung („voller Rust-Sibling") **nicht** ein.

## Entscheidung 2 — Wahrheit = **Dateisystembaum** (Inversion von „Log-als-Wahrheit")  ·  Reversibilität: Modell fix; Manifest-Format billig revidierbar

- **Invariante:** Die alleinige Wahrheit ist ein **echter Verzeichnisbaum** auf der
  Platte (§5). Ordner = Verzeichnisse, Dateien = Dateien; neben jedem Verzeichnis ein
  menschenlesbares `.scheme.json`-**Manifest** (Metadaten je Kind). Der Bestand ist
  **komplett lesbar** — man kann ihn per `ls` navigieren.
- **Begründung:** Die Nutzer-Anforderung „klare, menschlich navigierbare Struktur"
  ist am ehrlichsten erfüllt, wenn die Struktur **buchstäblich** das Dateisystem ist.
  Eine eingebettete KV-DB (wie redb bei lakearch) würde die Hierarchie in einer opaken
  Datei verstecken — das Gegenteil des Ziels.
- **Mutations-Disziplin:** **ein** serialisierter Writer; jeder Datei-/Manifest-
  Schreibvorgang ist **atomar** (Temp-Datei + `rename`), damit Leser nie einen halben
  Zustand sehen (§6.1).
- **Billig revidierbar:** Weil der Baum die Wahrheit ist, ist jeder Index (§8) ein
  wegwerf- und neu-baubares Derivat; das Manifest-Schema kann versioniert wachsen.

## Entscheidung 3 — Beschreibung = **Pflicht & maschinell erzwungen**  ·  Reversibilität: Politik revidierbar, Vertrag stabil

- **Wahl:** Jeder strukturierte Schreibvorgang trägt eine nicht-leere **Beschreibung**;
  der `Beschreibung`-Typ ist nur über einen prüfenden Konstruktor erzeugbar, der
  leere/nur-Leerraum-Texte ablehnt (§4). Der **Basis-Vertrag** (§6.4, Drop-in) fällt
  ohne Beschreibung auf einen `eingang/`-Ordner mit Platzhalter-Beschreibung +
  `unbeschrieben`-Markierung zurück.
- **Begründung:** Dies ist die konkrete, garantierte Umsetzung des Nutzer-Kerns
  „Daten liegen in klarer strukturierter Form vor **und müssen auch so beschrieben
  werden**". Der Leitfaden (§9) **lehrt** den Agenten, die Erzwingung **garantiert**
  es — Gürtel und Hosenträger.
- **scheme wertet die Beschreibung nicht** (§1.4): es prüft nur **Vorhandensein**,
  nie inhaltliche Güte.

## Entscheidung 4 — Austauschbarkeit-Seam = **`graveyard.Graveyard`** (aigentic/prizm)  ·  Reversibilität: revidierbar

- **Wahl:** „Austauschbar mit lakearch" bedeutet konkret: dasselbe `graveyard.Graveyard`-
  Interface der Schicht darüber erfüllen (`Put`/`Get`), über das aigentic heute schon
  `memory ↔ lakearch` tauscht. scheme wird der dritte Fall. Weil scheme veränderbar
  ist, erfüllt es zusätzlich die optionalen Fähigkeiten `Deletable`/`Listable`, die
  lakearch auslässt.
- **Anbindung:** wie lakearch über die C-ABI (`scheme-ffi`) und einen cgo-Adapter
  (`schemegrave`) hinter einem Build-Tag; der Daemon (`schemed`) für Netz-/Multi-Tenant.
- **Scheme-eigene, reichere Vorgänge** (Pfad + Pflicht-Beschreibung, Verschieben, der
  Leitfaden) liegen in schemes **eigenem** Interface, nicht im gemeinsamen Minimal-
  Vertrag — genau wie es das `graveyard`-Paketdoc für Backends mit eigenem Modell
  vorsieht.

---

## Reversibilitäts-Übersicht

| Entscheidung | Wahl | Reversibilität |
|---|---|---|
| Sprache | Rust (core/ffi/daemon wie lakearch) | schwer (Geschwister-Symmetrie); Flip nur bei Wegfall des Embeddings |
| Wahrheit | Dateisystembaum + `.scheme.json`-Manifeste | Modell fix; Manifest-Format/Index **billig revidierbar** |
| Beschreibung | Pflicht & erzwungen; `eingang`-Fallback am Basis-Seam | Vertrag stabil; Fallback-Politik revidierbar |
| Austauschbarkeit | `graveyard.Graveyard` + `Deletable`/`Listable`; FFI/Daemon | revidierbar (Topologie) |

---

## Folgen

- Das Substrat programmiert gegen **Pfade**, nicht gegen Inhalts-Hashes; es gibt
  **keine** automatische Deduplizierung (§3.4).
- Der Index (§8) bleibt ein reines Derivat; ein Wipe-&-Rebuild-aus-dem-Baum-Test
  friert diese Disziplin ein (Phase 2).
- Die aigentic-Anbindung ergänzt einen `case "scheme"` neben `memory`/`lakearch`;
  die Struktur-Leitung (§9) wird in die Kontext-Zusammenstellung gespritzt (Phase 5).
