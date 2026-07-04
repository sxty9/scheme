//! Das Datenmodell von scheme.
//!
//! Maßgeblich: Gesetzbuch `semantics/scheme.md`. Es gibt genau eine Entität, den
//! **Knoten** (§2), in genau zwei **Arten**: **Ordner** und **Datei**. Ein Knoten
//! ist über seinen **Pfad** identifiziert (§3) — menschenlesbar, hierarchisch,
//! stabil über Inhaltsänderungen. Jeder Knoten trägt eine **Beschreibung** (§4);
//! sie ist beim Schreiben Pflicht. scheme **wertet den Inhalt nicht** (§1) — es
//! hält Pfad, Art, Beschreibung und Metadaten rein strukturell.

use serde::{Deserialize, Serialize};

use crate::error::{Result, SchemeError};

/// Die Art eines Knotens (§2): ein **Ordner** (benannter Elternknoten, hält
/// Kinder) oder eine **Datei** (ein abgelegtes Datum, trägt Bytes an einem
/// Blattpfad).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Art {
    Ordner,
    Datei,
}

impl Art {
    /// Statischer Name für Fehlertexte und die C-ABI.
    pub fn name(self) -> &'static str {
        match self {
            Art::Ordner => "ordner",
            Art::Datei => "datei",
        }
    }
}

impl std::fmt::Display for Art {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Ein **Pfad** (§3): die Identität eines Knotens. Ein normalisierter, relativer,
/// mit `/` getrennter Pfad ohne führenden/abschließenden Schrägstrich, ohne leere
/// Segmente, ohne `.`/`..`. Der **Wurzelpfad** ist der leere Pfad (`""`) und
/// bezeichnet den Bestand selbst.
///
/// Der Typ **erzwingt** die Confinement-Regel bereits im Konstruktor: ein Pfad
/// kann nie aus dem Bestand herauszeigen (kein `..`, kein absoluter Pfad), daher
/// ist `wurzel.join(pfad)` immer innerhalb der Wurzel.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pfad(String);

impl Pfad {
    /// Der Wurzelpfad (der Bestand selbst).
    pub fn wurzel() -> Pfad {
        Pfad(String::new())
    }

    /// Parst und **validiert** einen Pfad (§3). Ein leerer Eingang ist die
    /// Wurzel. Führende/abschließende `/`, doppelte `/`, absolute Pfade sowie die
    /// Segmente `.` und `..` werden abgelehnt.
    pub fn parse(eingang: &str) -> Result<Pfad> {
        let s = eingang.trim();
        if s.is_empty() {
            return Ok(Pfad::wurzel());
        }
        if s.starts_with('/') {
            return Err(SchemeError::UngueltigerPfad(format!(
                "absoluter Pfad nicht erlaubt: {s:?}"
            )));
        }
        if s.ends_with('/') {
            return Err(SchemeError::UngueltigerPfad(format!(
                "abschließender Schrägstrich nicht erlaubt: {s:?}"
            )));
        }
        for seg in s.split('/') {
            Self::segment_pruefen(seg)?;
        }
        Ok(Pfad(s.to_string()))
    }

    /// Prüft ein einzelnes Segment (§3).
    fn segment_pruefen(seg: &str) -> Result<()> {
        if seg.is_empty() {
            return Err(SchemeError::UngueltigerPfad(
                "leeres Segment (doppelter Schrägstrich?)".to_string(),
            ));
        }
        if seg == "." || seg == ".." {
            return Err(SchemeError::UngueltigerPfad(format!(
                "reserviertes Segment nicht erlaubt: {seg:?}"
            )));
        }
        // Ein Segment enthält **nie** einen Trenner (`/`) oder `\`, keine NUL und
        // keine Steuerzeichen. Das `/`-Verbot ist tragend für das Confinement (§3):
        // `verketten` prüft sein Argument mit dieser Funktion, ohne selbst zu
        // splitten — ohne dieses Verbot ließe `verketten("../../x")` einen Pfad zu,
        // der aus der Wurzel herauszeigt.
        if seg
            .chars()
            .any(|c| c == '/' || c == '\\' || c == '\0' || c.is_control())
        {
            return Err(SchemeError::UngueltigerPfad(format!(
                "unzulässiges Zeichen in Segment: {seg:?}"
            )));
        }
        if seg.chars().all(char::is_whitespace) {
            return Err(SchemeError::UngueltigerPfad(
                "Segment besteht nur aus Leerraum".to_string(),
            ));
        }
        // Reservierte, scheme-interne Namen (§5): das Verzeichnis-Manifest und die
        // atomaren Schreib-Temporärdateien dürfen kein Knoten-Name sein. Der
        // Vergleich ist **case-insensitiv**, damit auch auf case-insensitiven
        // Dateisystemen (macOS/Windows) kein `.SCHEME.JSON` das Manifest verdrängt.
        let low = seg.to_ascii_lowercase();
        if low == ".scheme.json" || low.ends_with(".scheme-tmp") {
            return Err(SchemeError::UngueltigerPfad(format!(
                "reservierter interner Name nicht erlaubt: {seg:?}"
            )));
        }
        Ok(())
    }

    /// Ist dies der Wurzelpfad?
    pub fn ist_wurzel(&self) -> bool {
        self.0.is_empty()
    }

    /// Der Pfad als `/`-getrennte Zeichenkette (leer für die Wurzel).
    pub fn als_str(&self) -> &str {
        &self.0
    }

    /// Die Segmente des Pfades (leer für die Wurzel).
    pub fn segmente(&self) -> impl Iterator<Item = &str> {
        self.0.split('/').filter(|s| !s.is_empty())
    }

    /// Die Tiefe (Zahl der Segmente); die Wurzel hat Tiefe 0.
    pub fn tiefe(&self) -> usize {
        if self.ist_wurzel() {
            0
        } else {
            self.0.split('/').count()
        }
    }

    /// Das letzte Segment (der Name); `None` für die Wurzel.
    pub fn name(&self) -> Option<&str> {
        if self.ist_wurzel() {
            None
        } else {
            self.0.rsplit('/').next()
        }
    }

    /// Der Elternpfad; `None` für die Wurzel.
    pub fn elter(&self) -> Option<Pfad> {
        if self.ist_wurzel() {
            return None;
        }
        match self.0.rfind('/') {
            Some(i) => Some(Pfad(self.0[..i].to_string())),
            None => Some(Pfad::wurzel()),
        }
    }

    /// Hängt ein bereits validiertes Kindsegment an.
    pub fn verketten(&self, kindsegment: &str) -> Result<Pfad> {
        Pfad::segment_pruefen(kindsegment)?;
        if self.ist_wurzel() {
            Ok(Pfad(kindsegment.to_string()))
        } else {
            Ok(Pfad(format!("{}/{}", self.0, kindsegment)))
        }
    }

    /// Beginnt dieser Pfad mit `prefix` — an einer **Segmentgrenze**? Die Wurzel
    /// ist Präfix von allem. `a/b` ist Präfix von `a/b` und `a/b/c`, aber nicht
    /// von `a/bc`.
    pub fn beginnt_mit(&self, prefix: &Pfad) -> bool {
        if prefix.ist_wurzel() {
            return true;
        }
        if self.0 == prefix.0 {
            return true;
        }
        self.0.starts_with(&prefix.0) && self.0.as_bytes().get(prefix.0.len()) == Some(&b'/')
    }
}

impl std::fmt::Display for Pfad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Eine **Beschreibung** (§4): ein nicht-leerer, klarer Text, der einen
/// abgelegten Knoten strukturell einordnet. Der Typ ist der Ort, an dem die
/// Pflicht **maschinell** durchgesetzt wird: er ist nur über [`Beschreibung::neu`]
/// konstruierbar, und die lehnt leere/nur-Leerraum-Texte ab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Beschreibung(String);

impl Beschreibung {
    /// Baut eine Beschreibung; lehnt leere oder nur aus Leerraum bestehende
    /// Texte ab ([`SchemeError::BeschreibungFehlt`], §4).
    pub fn neu(text: impl Into<String>) -> Result<Beschreibung> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(SchemeError::BeschreibungFehlt);
        }
        Ok(Beschreibung(text))
    }

    /// Der Beschreibungstext.
    pub fn als_str(&self) -> &str {
        &self.0
    }

    /// Verbraucht die Beschreibung zum Text.
    pub fn in_string(self) -> String {
        self.0
    }
}

/// Die **Metadaten** eines Knotens — der Wert, den ein Verzeichnis-Manifest je
/// Kind hält (§5/§8). Serialisiert mit deutschen, menschenlesbaren Schlüsseln, so
/// dass ein Mensch das `.scheme.json` direkt lesen kann.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metadaten {
    /// Die Art des Knotens.
    pub art: Art,
    /// Die klare, menschenlesbare Beschreibung (§4). Immer nicht-leer.
    pub beschreibung: String,
    /// `true`, wenn die Beschreibung nur ein automatisch abgeleiteter Platzhalter
    /// ist (Basis-`ablegen_roh`/`eingang`-Fallback, §6.4) und noch strukturiert
    /// beschrieben werden sollte. Für ordentlich abgelegte Knoten `false`.
    #[serde(default)]
    pub unbeschrieben: bool,
    /// Optionaler MIME-Typ (aus der Dateiendung abgeleitet).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    /// Größe des Datei-Inhalts in Bytes (nur Datei).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groesse: Option<u64>,
    /// Optionale Herkunft (frei; z. B. Original-Dateiname oder Quelle).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quelle: Option<String>,
    /// Unix-Sekunden der Anlage.
    pub erstellt: u64,
    /// Unix-Sekunden der letzten Änderung.
    pub geaendert: u64,
}

/// Ein **Knoten** (§2), wie ihn eine Operation zurückgibt: sein Pfad, seine
/// Metadaten und — nur für eine Datei und nur wenn angefordert — sein Inhalt.
#[derive(Debug, Clone, PartialEq)]
pub struct Knoten {
    pub pfad: Pfad,
    pub metadaten: Metadaten,
    pub inhalt: Option<Vec<u8>>,
}

impl Knoten {
    /// Der Name (letztes Pfadsegment).
    pub fn name(&self) -> Option<&str> {
        self.pfad.name()
    }

    /// Die Art des Knotens.
    pub fn art(&self) -> Art {
        self.metadaten.art
    }

    /// Die Beschreibung des Knotens.
    pub fn beschreibung(&self) -> &str {
        &self.metadaten.beschreibung
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wurzel_hat_keinen_namen_und_kein_elter() {
        let w = Pfad::wurzel();
        assert!(w.ist_wurzel());
        assert_eq!(w.name(), None);
        assert_eq!(w.elter(), None);
        assert_eq!(w.tiefe(), 0);
    }

    #[test]
    fn pfad_parst_hierarchie() {
        let p = Pfad::parse("praesentationsraum/hardware/computer").unwrap();
        assert_eq!(p.als_str(), "praesentationsraum/hardware/computer");
        assert_eq!(p.name(), Some("computer"));
        assert_eq!(p.elter().unwrap().als_str(), "praesentationsraum/hardware");
        assert_eq!(p.tiefe(), 3);
        assert_eq!(
            p.segmente().collect::<Vec<_>>(),
            vec!["praesentationsraum", "hardware", "computer"]
        );
    }

    #[test]
    fn pfad_lehnt_traversal_und_absolut_ab() {
        assert!(Pfad::parse("/etc/passwd").is_err());
        assert!(Pfad::parse("a/../b").is_err());
        assert!(Pfad::parse("a//b").is_err());
        assert!(Pfad::parse("a/").is_err());
        assert!(Pfad::parse("a/.").is_err());
    }

    #[test]
    fn beginnt_mit_achtet_segmentgrenze() {
        let ab = Pfad::parse("a/b").unwrap();
        assert!(ab.beginnt_mit(&Pfad::parse("a").unwrap()));
        assert!(ab.beginnt_mit(&Pfad::wurzel()));
        assert!(ab.beginnt_mit(&ab));
        assert!(!ab.beginnt_mit(&Pfad::parse("a/bc").unwrap()));
        assert!(!Pfad::parse("ab").unwrap().beginnt_mit(&Pfad::parse("a").unwrap()));
    }

    #[test]
    fn beschreibung_ist_pflicht() {
        assert!(Beschreibung::neu("").is_err());
        assert!(Beschreibung::neu("   \n\t ").is_err());
        assert_eq!(
            Beschreibung::neu("Bedienungsanleitung des Computers")
                .unwrap()
                .als_str(),
            "Bedienungsanleitung des Computers"
        );
    }
}
