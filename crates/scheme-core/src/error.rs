//! Fehler des scheme-Kerns.
//!
//! Maßgeblich: Gesetzbuch `semantics/scheme.md`. scheme **wertet nicht** (§1) —
//! es meldet ausschließlich **mechanische**, strukturelle Fehlerzustände: eine
//! fehlende Pflicht-Beschreibung (§4), einen regelwidrigen Pfad (§3), einen
//! nicht vorhandenen Knoten, einen Art-Konflikt (Datei ↔ Ordner), einen
//! Betriebs-I/O-Fehler oder ein beschädigtes Verzeichnis-Manifest. Kein
//! Fehlerzustand trägt eine Wertung des Inhalts.

/// Mechanischer Fehlerzustand einer scheme-Operation. Jede Variante ist ein
/// struktureller oder operativer Zustand — **nie** eine Beurteilung der Daten.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SchemeError {
    /// Ein Schreibvorgang trägt **keine** oder eine nur aus Leerraum bestehende
    /// Beschreibung. Die Beschreibung ist Pflicht (§4): scheme legt niemals
    /// unbeschriebene, strukturlose Daten ab. Dies ist die maschinell erzwungene
    /// Umsetzung von „Daten müssen in klarer strukturierter Form beschrieben
    /// werden".
    #[error("Beschreibung fehlt oder ist leer (§4: Beschreibung ist Pflicht)")]
    BeschreibungFehlt,

    /// Der angegebene Pfad verletzt die Pfad-Regel (§3): er ist absolut, leer,
    /// enthält ein leeres Segment, `.` oder `..`, oder ein unzulässiges Zeichen.
    /// Der Grund ist rein strukturell.
    #[error("ungültiger Pfad (§3): {0}")]
    UngueltigerPfad(String),

    /// Der adressierte Knoten existiert nicht. Für Lese-Zugriffe ist Abwesenheit
    /// **kein** Fehler (sie liefert `None`); diese Variante entsteht nur, wo eine
    /// Operation einen vorhandenen Knoten voraussetzt (aktualisieren,
    /// verschieben, beschreiben).
    #[error("Knoten nicht vorhanden: {0}")]
    NichtGefunden(String),

    /// Art-Konflikt (§2): an dem Pfad liegt bereits ein Knoten der **anderen**
    /// Art (eine Datei, wo ein Ordner erwartet wird, oder umgekehrt). scheme
    /// wandelt eine Art nie still in die andere um.
    #[error("Art-Konflikt an {pfad}: erwartet {erwartet}, vorhanden {vorhanden}")]
    ArtKonflikt {
        pfad: String,
        erwartet: &'static str,
        vorhanden: &'static str,
    },

    /// Ein Ziel-Pfad ist bereits belegt, wo die Operation einen freien Pfad
    /// voraussetzt (Verschieben auf einen vorhandenen Knoten). scheme
    /// überschreibt beim Verschieben nicht still.
    #[error("Pfad bereits belegt: {0}")]
    BereitsVorhanden(String),

    /// Das `.scheme.json`-Manifest eines Verzeichnisses ist beschädigt oder mit
    /// dem Baum inkonsistent (§5/§8). scheme hält an, statt auf einem
    /// halb-gültigen Zustand fortzufahren.
    #[error("Verzeichnis-Manifest beschädigt: {0}")]
    ManifestBeschaedigt(String),

    /// Operativer I/O-Fehler des Dateisystembaums (§5-Persistenz): ein `read`,
    /// `write`, `rename`, `fsync` oder Verzeichnis-Vorgang ist fehlgeschlagen.
    #[error("I/O-Fehler des Dateisystembaums (§5)")]
    Io(#[source] std::io::Error),

    /// Fehler des **abgeleiteten** Index (§8). Der Index ist ein reines,
    /// neu-baubares Derivat des Baums — ein Index-Fehler bedroht die Wahrheit
    /// (den Baum) nicht; der Index kann verworfen und neu gebaut werden.
    #[error("Index-Fehler (§8): {0}")]
    IndexFehler(String),
}

impl From<std::io::Error> for SchemeError {
    fn from(e: std::io::Error) -> Self {
        SchemeError::Io(e)
    }
}

/// Bequemer Ergebnistyp der scheme-Operationen.
pub type Result<T> = std::result::Result<T, SchemeError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_is_send_sync() {
        // Fehler müssen über die eine serialisierte Schreib-Pipeline und Leser
        // hinweg transportierbar sein (Send + Sync).
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SchemeError>();
    }

    #[test]
    fn beschreibung_fehlt_hat_klaren_text() {
        assert!(SchemeError::BeschreibungFehlt.to_string().contains("§4"));
    }
}
