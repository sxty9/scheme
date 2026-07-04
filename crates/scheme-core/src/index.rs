//! Der **Index** (§8) — eine schnelle, **neu-baubare** Projektion des Baums.
//!
//! Maßgeblich: Gesetzbuch `semantics/scheme.md` §8. Der Index bildet `Pfad →
//! Metadaten` ab und erlaubt Präfix-Auflistung und eine Volltext-artige Suche
//! über die **Beschreibungen** — ohne den Baum ablaufen zu müssen. Er ist ein
//! **reines Derivat** (§8.2): er wird durch Ablaufen des [`Bestand`] erzeugt
//! ([`SchemeIndex::neu_bauen_aus`]) und kann jederzeit verworfen und neu gebaut
//! werden. Der Baum wird **nie** aus dem Index rekonstruiert — die Wahrheit ist
//! immer der Baum (§5).
//!
//! Der Index liegt bewusst **außerhalb** des Baums (eine eigene redb-Datei), damit
//! er nie mit den Daten verwechselt wird.

use std::path::Path;

use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};

use crate::error::{Result, SchemeError};
use crate::model::{Metadaten, Pfad};
use crate::store::Bestand;

/// Tabelle `Pfad (als String) → Metadaten (als JSON-String)`.
const PFAD_METADATEN: TableDefinition<&str, &str> = TableDefinition::new("pfad_metadaten");

fn ix(e: impl std::fmt::Display) -> SchemeError {
    SchemeError::IndexFehler(e.to_string())
}

/// Der abgeleitete Index eines Bestands (§8).
pub struct SchemeIndex {
    db: Database,
}

impl SchemeIndex {
    /// Öffnet (legt bei Bedarf an) einen Index unter `db_pfad`. Der Pfad sollte
    /// außerhalb der Bestands-Wurzel liegen.
    pub fn oeffnen(db_pfad: impl AsRef<Path>) -> Result<SchemeIndex> {
        let db = Database::create(db_pfad).map_err(ix)?;
        // Sicherstellen, dass die Tabelle existiert (leer), damit Lesevorgänge vor
        // dem ersten Bau nicht fehlschlagen.
        let wtx = db.begin_write().map_err(ix)?;
        {
            wtx.open_table(PFAD_METADATEN).map_err(ix)?;
        }
        wtx.commit().map_err(ix)?;
        Ok(SchemeIndex { db })
    }

    /// **Baut den Index neu aus dem Baum** (§8.2): wirft den bisherigen Inhalt weg
    /// und trägt jeden Knoten (Ordner und Datei) frisch ein. Nach einem Wipe des
    /// Index stellt dieser Aufruf den vollen Zustand byte-gleich wieder her.
    pub fn neu_bauen_aus(&mut self, bestand: &Bestand) -> Result<usize> {
        // Alle Knoten des Baums in deterministischer Pfad-Ordnung.
        let alle = bestand.traversieren(&Pfad::wurzel(), usize::MAX, usize::MAX)?;
        let wtx = self.db.begin_write().map_err(ix)?;
        // Tabelle leeren = löschen + neu anlegen (reines Derivat, §8.2).
        wtx.delete_table(PFAD_METADATEN).map_err(ix)?;
        let mut n = 0usize;
        {
            let mut t = wtx.open_table(PFAD_METADATEN).map_err(ix)?;
            for k in &alle {
                let json = serde_json::to_string(&k.metadaten).map_err(ix)?;
                t.insert(k.pfad.als_str(), json.as_str()).map_err(ix)?;
                n += 1;
            }
        }
        wtx.commit().map_err(ix)?;
        Ok(n)
    }

    /// Die Metadaten zu einem Pfad (oder `None`), aus dem Index.
    pub fn metadaten(&self, pfad: &Pfad) -> Result<Option<Metadaten>> {
        let rtx = self.db.begin_read().map_err(ix)?;
        let t = rtx.open_table(PFAD_METADATEN).map_err(ix)?;
        match t.get(pfad.als_str()).map_err(ix)? {
            Some(v) => {
                let meta = serde_json::from_str(v.value()).map_err(ix)?;
                Ok(Some(meta))
            }
            None => Ok(None),
        }
    }

    /// Alle indizierten Pfade unterhalb (bzw. gleich) `prefix`, sortiert.
    pub fn unter_prefix(&self, prefix: &Pfad) -> Result<Vec<Pfad>> {
        let rtx = self.db.begin_read().map_err(ix)?;
        let t = rtx.open_table(PFAD_METADATEN).map_err(ix)?;
        let mut out = Vec::new();
        for eintrag in t.iter().map_err(ix)? {
            let (k, _v) = eintrag.map_err(ix)?;
            let p = Pfad::parse(k.value())?;
            if p.beginnt_mit(prefix) && !p.ist_wurzel() {
                out.push(p);
            }
        }
        out.sort();
        Ok(out)
    }

    /// Alle Pfade, deren **Beschreibung** `nadel` enthält (Groß-/Kleinschreibung
    /// ignoriert), sortiert. Reines Substring-Matching — scheme wertet nicht (§1.4).
    pub fn suche_beschreibung(&self, nadel: &str) -> Result<Vec<Pfad>> {
        let nadel = nadel.to_lowercase();
        let rtx = self.db.begin_read().map_err(ix)?;
        let t = rtx.open_table(PFAD_METADATEN).map_err(ix)?;
        let mut out = Vec::new();
        for eintrag in t.iter().map_err(ix)? {
            let (k, v) = eintrag.map_err(ix)?;
            let meta: Metadaten = serde_json::from_str(v.value()).map_err(ix)?;
            if meta.beschreibung.to_lowercase().contains(&nadel) {
                out.push(Pfad::parse(k.value())?);
            }
        }
        out.sort();
        Ok(out)
    }

    /// Die Zahl der indizierten Einträge (für Tests/Betrieb).
    pub fn anzahl(&self) -> Result<usize> {
        let rtx = self.db.begin_read().map_err(ix)?;
        let t = rtx.open_table(PFAD_METADATEN).map_err(ix)?;
        Ok(t.len().map_err(ix)? as usize)
    }
}
