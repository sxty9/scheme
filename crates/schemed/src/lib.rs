//! schemed — der **scheme-Daemon** (Bibliotheks-Kern).
//!
//! Maßgeblich: das Gesetzbuch `semantics/scheme.md`. Der Daemon besitzt **einen**
//! Bestand (= **ein** [`scheme_core::Bestand`]) und exponiert die veränderbaren,
//! pfadbasierten scheme-Primitive (§6/§7) über das Netz (gRPC/tonic) — für
//! Netz-/Multi-Tenant-Einsatz, wo die C-ABI (`scheme-ffi`, eine Vertrauenszone)
//! nicht genügt.
//!
//! ## Was der Rand exponiert — und ausschließlich (§1.4)
//!
//! Genau die scheme-Primitive: ablegen (strukturiert §6, oder roh §6.4), lesen,
//! aktualisieren, verschieben, löschen, beschreiben, auflisten und den Leitfaden
//! (§9). **Kein** Sortieren/Aggregieren/Ranken/Rechnen am Rand — scheme wertet
//! nicht.
//!
//! ## Sync-Kern, async-Rand
//!
//! [`scheme_core::Bestand`] ist **sync** und serialisiert Schreibvorgänge bereits
//! intern (ein Writer hinter einer Sperre, §6.1). Der gRPC-Rand ist `tokio`; jede
//! Operation läuft über [`tokio::task::spawn_blocking`] auf dem geteilten Bestand
//! (viele nebenläufige Leser; die interne Sperre linearisiert die Schreiber).

pub mod bestand;
pub mod service;

/// Der von `tonic-prost-build` aus `proto/scheme.proto` generierte Code. Die
/// Wire-Form ist eine Rand-Entscheidung (§11.3), nicht das Substrat.
pub mod proto {
    #![allow(clippy::doc_overindented_list_items)]
    tonic::include_proto!("scheme.v1");
}

pub use bestand::Bestand;
pub use service::SchemeService;
