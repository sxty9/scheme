//! scheme-core — das Substrat.
//!
//! Maßgeblich ist das Gesetzbuch `semantics/scheme.md`. Dieses Crate implementiert
//! den scheme-Datenpool: einen **veränderbaren, klar strukturierten**
//! Dateisystembaum-als-Wahrheit (§5), in dem Daten und Ordner in einer
//! menschlich navigierbaren Hierarchie liegen (z. B.
//! `praesentationsraum/hardware/computer/bedienungsanleitung.pdf`), adressiert über
//! **Pfade** (§3) statt Inhalts-Hashes. Anders als lakearch (strikt append-only,
//! wild hash-adressiert) ist scheme **voll lesbar und schreibbar**: ablegen,
//! aktualisieren, verschieben, löschen sind erststellige Primitive (§6).
//!
//! Wie lakearch **wertet scheme nicht** (§1): es speichert, strukturiert und liest
//! zurück — es beurteilt/sortiert/rechnet den Inhalt nie. Seine eine aktive
//! Besonderheit ist der **Leitfaden** (§9): zur Traversier-/Schreibzeit teilt
//! scheme dem Agenten mit, dass die Daten klar strukturiert vorliegen und auch so
//! **beschrieben werden müssen** — und es erzwingt die **Pflicht-Beschreibung**
//! (§4) an der Schreib-Grenze.
//!
//! ## Module
//!
//! - [`model`] — das Datenmodell (§2/§3/§4): [`Pfad`] (Identität), [`Art`]
//!   (Ordner/Datei), die pflichtige [`Beschreibung`], [`Metadaten`] und [`Knoten`].
//! - [`store`] — der [`Bestand`] (§5/§6/§7): der Dateisystembaum-als-Wahrheit mit
//!   `.scheme.json`-Manifesten, serialisiertem Writer und atomaren Schreibvorgängen;
//!   die Primitive `ablegen`/`ablegen_roh`/`aktualisieren`/`verschieben`/`loeschen`/
//!   `beschreiben`/`lesen`/`knoten`/`auflisten`/`kinder`/`traversieren` und der
//!   [`Bestand::leitfaden`] (§9).
//! - [`index`] — der [`SchemeIndex`] (§8): eine **neu-baubare** redb-Projektion
//!   `Pfad → Metadaten` für Präfix-Auflistung und Beschreibungs-Suche; reines
//!   Derivat des Baums, jederzeit verwerf- und neu baubar.
//! - [`error`] — [`SchemeError`]: rein **mechanische** Zustände (§1), inkl. der
//!   erzwungenen [`SchemeError::BeschreibungFehlt`] (§4).

pub mod error;
pub mod index;
pub mod model;
pub mod store;

pub use error::{Result, SchemeError};
pub use index::SchemeIndex;
pub use model::{Art, Beschreibung, Knoten, Metadaten, Pfad};
pub use store::{Bestand, EINGANG, LEITFADEN, MANIFEST_NAME};

/// Der Struktur-Leitfaden (§9): der Text, mit dem scheme dem Agenten mitteilt,
/// dass die Daten klar strukturiert vorliegen und auch so beschrieben werden
/// müssen. Frei zugänglich (auch ohne offenen Bestand), damit die C-ABI und der
/// Daemon ihn statisch nach außen reichen können.
pub fn leitfaden() -> &'static str {
    LEITFADEN
}
