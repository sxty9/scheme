# scheme

**Ein veränderbarer, klar strukturierter Datenpool — der Gegenpol zu [lakearch](../lakearch).**

scheme agiert auf derselben Ebene wie lakearch und ist mit ihr **austauschbar** (über
das `graveyard.Graveyard`-Interface der aigentic-/prizm-Schicht). Der Unterschied ist
der Charakter der Ablage:

| | lakearch | **scheme** |
|---|---|---|
| Veränderbarkeit | strikt **append-only** | **voll lesbar & schreibbar** (put/update/move/delete) |
| Struktur | „wilder", per Hash adressierter Graph | **klare, menschlich navigierbare Hierarchie** (echte Ordner/Dateien) |
| Adressierung | Inhalts-Hash (BLAKE3) | **Pfad = Identität** (`praesentationsraum/hardware/computer/…`) |
| Wahrheit | Append-Log | **Dateisystembaum** (`ls`-bar, „komplett lesbar") |
| Besonderheit | — | **Pflicht-Beschreibung** + **Leitfaden** für den Agenten |

Wie lakearch **wertet scheme nicht** (rechnet/sortiert/beurteilt den Inhalt nie). Als
Traverser und Datenschreiber agiert eine **aigentic-Instanz**; scheme teilt ihr zur
Schreib-/Traversierzeit mit, dass die Daten klar strukturiert vorliegen und **auch so
beschrieben werden müssen** — und erzwingt die Beschreibung maschinell.

Das maßgebliche, vollständige Regelwerk ist [`semantics/scheme.md`](semantics/scheme.md)
(axiomatisch, §1–§12). Architektur-Entscheidungen: [`docs/adr/0001`](docs/adr/0001-substrat-technologie.md).

## Aufbau (Rust-Workspace, wie lakearch)

- **`crates/scheme-core`** — das Substrat: der `Bestand` (Dateisystembaum-als-Wahrheit
  mit `.scheme.json`-Manifesten, serialisiertem Writer, atomaren Schreibvorgängen), das
  Modell (`Pfad`/`Art`/`Beschreibung`/`Knoten`), der wieder-baubare redb-`SchemeIndex`.
- **`crates/scheme-ffi`** — die C-ABI (`scheme_ffi.h`) für das In-Prozess-Embedding
  (opake Handles, `catch_unwind`, Puffer-Protokoll). Baut `libscheme_ffi.{a,so}`.
- **`crates/schemed`** — der gRPC-Daemon (tonic) für Netz-/Multi-Tenant-Einsatz.

Die Anbindung an aigentic lebt dort (Sibling-Repo): `graveyard/schemegrave` (cgo über
die C-ABi) implementiert `graveyard.Graveyard` + `Deletable` + `Listable` + schemes
eigene `Structured`-Oberfläche; auswählbar über `AIGENTIC_GRAVEYARD=scheme` (Build mit
`-tags scheme`).

## Bauen & Testen

```sh
cargo build                 # Workspace (core + ffi + daemon)
cargo test                  # alle Tests
cargo clippy --all-targets -- -D warnings

# Release-FFI-Bibliothek (die der cgo-Adapter in aigentic linkt):
cargo build -p scheme-ffi --release   # -> target/release/libscheme_ffi.a

# Daemon starten:
cargo run -p schemed -- ./scheme-data # gRPC auf 127.0.0.1:50052 (SCHEMED_ADDR)
```

Man kann den Bestand direkt inspizieren — er ist ein echter Verzeichnisbaum:

```sh
ls -R ./scheme-data         # Ordner/Dateien; je Verzeichnis ein .scheme.json
```
