//! Die **C-ABI-Fehlercodes** (`SchemeStatus`) der FFI-Schicht.
//!
//! Maßgeblich: das Gesetzbuch `semantics/scheme.md`. Die Grenze gibt **rein
//! mechanische** Zustände als kleine `int32`-Codes heraus — **niemals** eine
//! Wertung des Inhalts (§1.4). Ein [`SchemeError`] wird in **genau einen** Code
//! abgebildet; ein über die C-ABI durchgereichter Rust-Panic (UB!) wird als
//! [`SchemeStatus::Panic`] zurückgegeben, **nie** als Unwind.

use scheme_core::SchemeError;

/// Fehlercode jeder `extern "C"`-Funktion (`int32`, C-ABI-stabil).
///
/// `Ok = 0`; alles andere ist ein definierter Fehlerzustand. Die Werte sind Teil
/// des ABI-Vertrags und dürfen sich nicht verschieben (append-only erweitern, nie
/// umnummerieren).
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SchemeStatus {
    /// Erfolg.
    Ok = 0,
    /// Ein über die C-ABI gefangener **Panic** (`catch_unwind`): statt UB → Code.
    Panic = 1,
    /// Ein **Null-Pointer** kam, wo ein gültiger Pointer verlangt war, oder ein
    /// Längen-/Argument-Vertrag wurde verletzt.
    NullArgument = 2,
    /// Ein übergebenes Handle ist ungültig/bereits geschlossen.
    InvalidHandle = 3,
    /// Der adressierte Knoten ist **nicht vorhanden** (Lesen eines fehlenden
    /// Knotens, oder eine Operation, die einen vorhandenen voraussetzt).
    NotFound = 4,
    /// Der vom Aufrufer bereitgestellte Puffer war **zu klein**; die benötigte
    /// Länge wird über `out_len` zurückgemeldet (erneut mit größerem Puffer).
    BufferTooSmall = 5,

    // -- Abbildung der mechanischen `SchemeError`-Zustände ----------------------
    /// Die Pflicht-**Beschreibung** fehlt/ist leer (§4) — maschinell erzwungen.
    DescriptionRequired = 6,
    /// Der Pfad verletzt die Pfad-Regel (§3): absolut, `..`, leeres Segment, …
    InvalidPath = 7,
    /// Art-Konflikt (§2): Datei ↔ Ordner an demselben Pfad.
    KindConflict = 8,
    /// Der Ziel-Pfad ist bereits belegt (Verschieben auf einen vorhandenen Knoten).
    AlreadyExists = 9,
    /// Operativer I/O-Fehler des Dateisystembaums (§5).
    Io = 14,
    /// Ein `.scheme.json`-Manifest ist beschädigt/inkonsistent (§5/§8); HALT.
    ManifestCorrupt = 15,
    /// Fehler des abgeleiteten Index (§8) — der Baum (die Wahrheit) ist unberührt.
    IndexError = 16,
    /// Ein hier (noch) nicht kategorisierter mechanischer Zustand.
    Other = 99,
}

impl SchemeStatus {
    /// Der rohe `int32`-Code (für Tests und die `extern "C"`-Rückgabe).
    pub fn code(self) -> i32 {
        self as i32
    }
}

impl From<SchemeError> for SchemeStatus {
    /// Bildet einen mechanischen [`SchemeError`] in **genau einen** C-ABI-Code ab.
    fn from(e: SchemeError) -> Self {
        match e {
            SchemeError::BeschreibungFehlt => SchemeStatus::DescriptionRequired,
            SchemeError::UngueltigerPfad(_) => SchemeStatus::InvalidPath,
            SchemeError::NichtGefunden(_) => SchemeStatus::NotFound,
            SchemeError::ArtKonflikt { .. } => SchemeStatus::KindConflict,
            SchemeError::BereitsVorhanden(_) => SchemeStatus::AlreadyExists,
            SchemeError::ManifestBeschaedigt(_) => SchemeStatus::ManifestCorrupt,
            SchemeError::Io(_) => SchemeStatus::Io,
            SchemeError::IndexFehler(_) => SchemeStatus::IndexError,
            // `SchemeError` ist `#[non_exhaustive]`: ein künftiger, hier unbekannter
            // Zustand ⇒ Other, nie ein stiller Erfolg.
            _ => SchemeStatus::Other,
        }
    }
}
