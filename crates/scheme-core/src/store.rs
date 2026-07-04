//! Der **Bestand** — der scheme-Store als Dateisystembaum-als-Wahrheit (§5).
//!
//! Maßgeblich: Gesetzbuch `semantics/scheme.md`. Die **Wahrheit** ist ein echter
//! Verzeichnisbaum auf der Platte (§5): Ordner sind echte Verzeichnisse, Dateien
//! echte Dateien — ein Mensch kann den Bestand per `ls` navigieren. Neben jedem
//! Verzeichnis liegt ein `.scheme.json`-**Manifest**, das jedes Kind auf seine
//! Metadaten (Art, **Beschreibung**, Größe, Zeitstempel …) abbildet.
//!
//! Alle Mutationen (§6) laufen durch **einen serialisierten Writer**
//! ([`Bestand::schreib_sperre`]); Datei- und Manifest-Schreibvorgänge sind
//! **atomar** (Temp-Datei + `rename`), so dass Leser nie einen halb-geschriebenen
//! Zustand sehen. scheme **wertet nicht** (§1): es speichert, strukturiert und
//! liest zurück — es beurteilt den Inhalt nie.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{Result, SchemeError};
use crate::model::{Art, Beschreibung, Knoten, Metadaten, Pfad};

/// Name des Verzeichnis-Manifests (§5). Reserviert (siehe `Pfad::segment_pruefen`).
pub const MANIFEST_NAME: &str = ".scheme.json";

/// Der wohlbekannte Eingangs-Ordner (§6.4): das Basis-`ablegen_roh` ohne Pfad legt
/// hier ab, mit abgeleiteter Platzhalter-Beschreibung, bis der Agent es
/// strukturiert einordnet.
pub const EINGANG: &str = "eingang";

/// Der **Leitfaden** (§9): der feste Text, mit dem scheme dem traversierenden und
/// schreibenden Agenten mitteilt, dass die Daten klar strukturiert vorliegen und
/// auch so beschrieben werden **müssen**. Wird über die C-ABI (`scheme_describe`)
/// und den Daemon nach außen gereicht und von der aigentic-Kontext-Zusammenstellung
/// in den Agenten-Kontext gespritzt.
pub const LEITFADEN: &str = "\
scheme ist ein veränderbarer, menschlich navigierbarer, klar strukturierter Datenpool. \
Alle Daten liegen in einer eindeutigen, hierarchischen Ordnerstruktur \
(z. B. praesentationsraum/hardware/computer/bedienungsanleitung.pdf). \
Beim Ablegen MUSST du (a) einen klaren strukturellen Pfad wählen, der den Ort im \
Baum eindeutig macht, und (b) eine klare, menschenlesbare Beschreibung angeben, die \
sagt, was das Datum ist und wohin es gehört. Lege niemals unbeschriftete oder \
unstrukturierte Blobs ab. scheme wertet den Inhalt nicht — die Struktur und die \
Beschreibung gibst du vor.";

fn eins() -> u32 {
    1
}

/// Ein Verzeichnis-Manifest (`.scheme.json`): die Metadaten je Kind. `BTreeMap`
/// hält die Schlüssel sortiert ⇒ stabile, menschenlesbare Diffs.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    #[serde(default = "eins")]
    version: u32,
    #[serde(default)]
    kinder: BTreeMap<String, Metadaten>,
}

impl Manifest {
    fn leer() -> Manifest {
        Manifest {
            version: 1,
            kinder: BTreeMap::new(),
        }
    }
}

/// Der scheme-Store: ein Dateisystembaum-als-Wahrheit unter genau einer Wurzel,
/// hinter einem serialisierten Writer (§6).
pub struct Bestand {
    wurzel: PathBuf,
    schreib_sperre: Mutex<()>,
}

impl Bestand {
    /// Öffnet (legt bei Bedarf an) einen Bestand mit Wurzel `wurzel`.
    pub fn oeffnen(wurzel: impl Into<PathBuf>) -> Result<Bestand> {
        let wurzel = wurzel.into();
        fs::create_dir_all(&wurzel)?;
        let b = Bestand {
            wurzel,
            schreib_sperre: Mutex::new(()),
        };
        let rp = manifest_pfad(&b.wurzel);
        if !rp.exists() {
            b.manifest_schreiben(&b.wurzel, &Manifest::leer())?;
        }
        Ok(b)
    }

    /// Die Wurzel des Bestands auf der Platte.
    pub fn wurzel(&self) -> &Path {
        &self.wurzel
    }

    /// Der Struktur-Leitfaden (§9).
    pub fn leitfaden(&self) -> &'static str {
        LEITFADEN
    }

    // ----------------------------------------------------------------- Schreiben

    /// **Strukturiert ablegen** (§6): schreibt `inhalt` als Datei an `pfad` mit
    /// der **Pflicht-Beschreibung**. Legt fehlende Elternordner an. Überschreibt
    /// eine vorhandene Datei am Pfad in-place (die Beschreibung wird gesetzt).
    pub fn ablegen(&self, pfad: &Pfad, beschreibung: Beschreibung, inhalt: &[u8]) -> Result<Knoten> {
        let _sperre = self.schreib_sperre.lock().expect("Schreib-Sperre vergiftet");
        self.schreibe_datei(pfad, beschreibung.in_string(), false, None, inhalt)
    }

    /// **Roh ablegen** (Basis-Vertrag, §6.4): der Drop-in-Pfad ohne Beschreibung.
    /// Bei leerem/Wurzel-Pfad wird deterministisch unter `eingang/` abgelegt; bei
    /// gegebenem Pfad überschrieben. Eine vorhandene Beschreibung bleibt erhalten;
    /// sonst wird eine Platzhalter-Beschreibung abgeleitet und `unbeschrieben`
    /// gesetzt. So bleibt scheme ein gültiges Drop-in am Minimal-Seam, ohne die
    /// „muss beschrieben werden"-Zusage aufzugeben (der strukturierte Pfad ist der
    /// eigentlich intendierte).
    pub fn ablegen_roh(&self, pfad: Option<&Pfad>, inhalt: &[u8]) -> Result<Knoten> {
        let _sperre = self.schreib_sperre.lock().expect("Schreib-Sperre vergiftet");
        let pfad = match pfad {
            Some(p) if !p.ist_wurzel() => p.clone(),
            _ => {
                let name = format!("{}.bin", hex8(blake3::hash(inhalt).as_bytes()));
                Pfad::wurzel().verketten(EINGANG)?.verketten(&name)?
            }
        };
        let name = pfad.name().expect("nicht-Wurzel-Pfad hat einen Namen");
        let elter_abs = self.abs(&pfad.elter().expect("nicht-Wurzel-Pfad hat einen Elter"));
        let vorhanden = self
            .manifest_lesen(&elter_abs)
            .ok()
            .and_then(|m| m.kinder.get(name).cloned());
        let (beschreibung, unbeschrieben) = match vorhanden {
            Some(e) if e.art == Art::Datei => (e.beschreibung, e.unbeschrieben),
            _ => (stub_text(name), true),
        };
        self.schreibe_datei(&pfad, beschreibung, unbeschrieben, None, inhalt)
    }

    /// **Aktualisieren** (§6): überschreibt den Inhalt einer **vorhandenen** Datei;
    /// die Beschreibung bleibt erhalten. Fehlt der Knoten ⇒ [`SchemeError::NichtGefunden`].
    pub fn aktualisieren(&self, pfad: &Pfad, inhalt: &[u8]) -> Result<Knoten> {
        let _sperre = self.schreib_sperre.lock().expect("Schreib-Sperre vergiftet");
        let name = pfad
            .name()
            .ok_or_else(|| SchemeError::UngueltigerPfad("die Wurzel ist keine Datei".into()))?;
        let elter_abs = self.abs(&pfad.elter().unwrap());
        let mut m = self.manifest_lesen(&elter_abs)?;
        let e = m
            .kinder
            .get(name)
            .ok_or_else(|| SchemeError::NichtGefunden(pfad.to_string()))?;
        if e.art != Art::Datei {
            return Err(SchemeError::ArtKonflikt {
                pfad: pfad.to_string(),
                erwartet: "datei",
                vorhanden: e.art.name(),
            });
        }
        self.atomar_schreiben(&self.abs(pfad), inhalt)?;
        let mut meta = e.clone();
        meta.groesse = Some(inhalt.len() as u64);
        meta.geaendert = jetzt();
        m.kinder.insert(name.to_string(), meta.clone());
        self.manifest_schreiben(&elter_abs, &m)?;
        Ok(Knoten {
            pfad: pfad.clone(),
            metadaten: meta,
            inhalt: None,
        })
    }

    /// **Verschieben / Umbenennen** (§6): re-keyt einen Knoten (Datei oder Ordner)
    /// von `von` nach `nach`. `nach` muss frei sein; fehlende Ziel-Elternordner
    /// werden angelegt. Der Inhalt und die Beschreibung wandern mit.
    pub fn verschieben(&self, von: &Pfad, nach: &Pfad) -> Result<()> {
        let _sperre = self.schreib_sperre.lock().expect("Schreib-Sperre vergiftet");
        if von.ist_wurzel() || nach.ist_wurzel() {
            return Err(SchemeError::UngueltigerPfad(
                "die Wurzel kann nicht verschoben werden".into(),
            ));
        }
        let von_name = von.name().unwrap();
        let nach_name = nach.name().unwrap();
        let von_elter = von.elter().unwrap();
        let nach_elter = nach.elter().unwrap();
        let von_elter_abs = self.abs(&von_elter);

        // 1. Die Quelle muss existieren — nur die Metadaten kopieren, das Manifest
        //    NICHT über Schritt 3 hinweg festhalten (§6).
        let meta = self
            .manifest_lesen(&von_elter_abs)?
            .kinder
            .get(von_name)
            .ok_or_else(|| SchemeError::NichtGefunden(von.to_string()))?
            .clone();

        // 2. Das Ziel muss frei sein.
        let nach_abs = self.abs(nach);
        if nach_abs.exists() {
            return Err(SchemeError::BereitsVorhanden(nach.to_string()));
        }

        // 3. Ziel-Ordnerkette anlegen. ACHTUNG: dies kann Vorfahren-Manifeste
        //    verändern — einschließlich `von_elter` (etwa beim Verschieben in einen
        //    NEUEN Unterordner desselben Elternverzeichnisses). Deshalb werden die
        //    Manifeste in Schritt 5 **frisch** gelesen (statt eine vor Schritt 3
        //    gelesene, jetzt veraltete Kopie zurückzuschreiben und den soeben
        //    angelegten Ordner-Eintrag zu überschreiben).
        self.ordnerkette_sicherstellen(&nach_elter)?;

        // 4. Auf der Platte umbenennen (bewegt bei einem Ordner den ganzen Teilbaum).
        fs::rename(self.abs(von), &nach_abs)?;

        // 5. Manifeste aktualisieren — FRISCH gelesen nach Schritt 3.
        let mut neu_meta = meta;
        neu_meta.geaendert = jetzt();
        if von_elter == nach_elter {
            let mut m = self.manifest_lesen(&von_elter_abs)?;
            m.kinder.remove(von_name);
            m.kinder.insert(nach_name.to_string(), neu_meta);
            self.manifest_schreiben(&von_elter_abs, &m)?;
        } else {
            let mut von_m = self.manifest_lesen(&von_elter_abs)?;
            von_m.kinder.remove(von_name);
            self.manifest_schreiben(&von_elter_abs, &von_m)?;
            let nach_elter_abs = self.abs(&nach_elter);
            let mut nach_m = self.manifest_lesen(&nach_elter_abs)?;
            nach_m.kinder.insert(nach_name.to_string(), neu_meta);
            self.manifest_schreiben(&nach_elter_abs, &nach_m)?;
        }
        Ok(())
    }

    /// **Löschen** (§6): entfernt einen Knoten (eine Datei oder rekursiv einen
    /// Ordner). Idempotent: ein abwesender Knoten liefert `false` ohne Fehler.
    pub fn loeschen(&self, pfad: &Pfad) -> Result<bool> {
        let _sperre = self.schreib_sperre.lock().expect("Schreib-Sperre vergiftet");
        if pfad.ist_wurzel() {
            return Err(SchemeError::UngueltigerPfad(
                "die Wurzel kann nicht gelöscht werden".into(),
            ));
        }
        let name = pfad.name().unwrap();
        let elter_abs = self.abs(&pfad.elter().unwrap());
        let mut m = self.manifest_lesen(&elter_abs)?;
        let im_manifest = m.kinder.remove(name).is_some();
        // Zuerst aus dem Manifest entfernen (aus der Auflistung nehmen), DANN von der
        // Platte: ein Absturz dazwischen hinterlässt höchstens eine verwaiste,
        // nicht-gelistete Datei (harmlos) — nie einen „Phantom"-Eintrag, der gelistet
        // wird, aber keine Datei mehr hat.
        if im_manifest {
            self.manifest_schreiben(&elter_abs, &m)?;
        }
        let abs = self.abs(pfad);
        let im_fs = abs.exists();
        if abs.is_dir() {
            fs::remove_dir_all(&abs)?;
        } else if im_fs {
            fs::remove_file(&abs)?;
        }
        Ok(im_manifest || im_fs)
    }

    /// **Beschreiben** (§4): setzt/ersetzt die Beschreibung eines vorhandenen
    /// Knotens und löscht das `unbeschrieben`-Flag. So kann der Agent einen per
    /// `eingang`-Fallback abgelegten Knoten nachträglich klar einordnen.
    pub fn beschreiben(&self, pfad: &Pfad, beschreibung: Beschreibung) -> Result<Knoten> {
        let _sperre = self.schreib_sperre.lock().expect("Schreib-Sperre vergiftet");
        let name = pfad
            .name()
            .ok_or_else(|| SchemeError::UngueltigerPfad("die Wurzel hat keine Beschreibung".into()))?;
        let elter_abs = self.abs(&pfad.elter().unwrap());
        let mut m = self.manifest_lesen(&elter_abs)?;
        let e = m
            .kinder
            .get_mut(name)
            .ok_or_else(|| SchemeError::NichtGefunden(pfad.to_string()))?;
        e.beschreibung = beschreibung.in_string();
        e.unbeschrieben = false;
        e.geaendert = jetzt();
        let meta = e.clone();
        self.manifest_schreiben(&elter_abs, &m)?;
        Ok(Knoten {
            pfad: pfad.clone(),
            metadaten: meta,
            inhalt: None,
        })
    }

    // -------------------------------------------------------------------- Lesen

    /// **Lesen** (§7): der Inhalt der Datei an `pfad`. Abwesenheit (oder ein
    /// Ordner) liefert `None` — Abwesenheit ist ein normales Ergebnis, kein Fehler.
    pub fn lesen(&self, pfad: &Pfad) -> Result<Option<Vec<u8>>> {
        if pfad.ist_wurzel() {
            return Ok(None);
        }
        let abs = self.abs(pfad);
        if abs.is_dir() {
            return Ok(None);
        }
        match fs::read(&abs) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Der **Knoten** an `pfad` mit seinen Metadaten (§7); `mit_inhalt` lädt bei
    /// einer Datei zusätzlich die Bytes. `None`, wenn nichts an dem Pfad liegt.
    pub fn knoten(&self, pfad: &Pfad, mit_inhalt: bool) -> Result<Option<Knoten>> {
        let Some(name) = pfad.name() else {
            return Ok(None);
        };
        let elter_abs = self.abs(&pfad.elter().unwrap());
        let m = self.manifest_lesen(&elter_abs)?;
        let Some(meta) = m.kinder.get(name).cloned() else {
            return Ok(None);
        };
        let inhalt = if mit_inhalt && meta.art == Art::Datei {
            self.lesen(pfad)?
        } else {
            None
        };
        Ok(Some(Knoten {
            pfad: pfad.clone(),
            metadaten: meta,
            inhalt,
        }))
    }

    /// **Auflisten** (§7): alle Datei-Pfade unterhalb von `prefix` (rekursiv),
    /// sortiert. Das sind genau die per [`Bestand::lesen`] auflösbaren Referenzen
    /// (die Ordner-Struktur liefert [`Bestand::kinder`]/[`Bestand::traversieren`]).
    pub fn auflisten(&self, prefix: &Pfad) -> Result<Vec<Pfad>> {
        let mut out = Vec::new();
        self.sammle_dateien(prefix, &mut out)?;
        out.sort();
        Ok(out)
    }

    /// Die **direkten Kinder** eines Ordners (§7), als Knoten (ohne Inhalt),
    /// sortiert nach Pfad.
    pub fn kinder(&self, pfad: &Pfad) -> Result<Vec<Knoten>> {
        let m = self.manifest_lesen(&self.abs(pfad))?;
        let mut out = Vec::with_capacity(m.kinder.len());
        for (name, meta) in &m.kinder {
            out.push(Knoten {
                pfad: pfad.verketten(name)?,
                metadaten: meta.clone(),
                inhalt: None,
            });
        }
        out.sort_by(|a, b| a.pfad.cmp(&b.pfad));
        Ok(out)
    }

    /// **Traversieren** (§7): eine beschränkte (Tiefe/Knoten-Budget), determin-
    /// istische Pre-Order-Traversierung ab `start`. Der Baum ist azyklisch, daher
    /// terminiert die Traversierung strukturell; die Budgets schützen zusätzlich
    /// vor unbeschränktem Speicher. scheme ordnet die Schritte deterministisch
    /// nach Pfad — es rankt/wertet nicht (§1).
    pub fn traversieren(
        &self,
        start: &Pfad,
        max_tiefe: usize,
        max_knoten: usize,
    ) -> Result<Vec<Knoten>> {
        let mut out = Vec::new();
        let mut budget = max_knoten;
        self.walk(start, 0, max_tiefe, &mut budget, &mut out)?;
        Ok(out)
    }

    fn walk(
        &self,
        dir: &Pfad,
        tiefe: usize,
        max_tiefe: usize,
        budget: &mut usize,
        out: &mut Vec<Knoten>,
    ) -> Result<()> {
        if tiefe >= max_tiefe || *budget == 0 {
            return Ok(());
        }
        for k in self.kinder(dir)? {
            if *budget == 0 {
                break;
            }
            let ist_ordner = k.art() == Art::Ordner;
            let kp = k.pfad.clone();
            out.push(k);
            *budget -= 1;
            if ist_ordner {
                self.walk(&kp, tiefe + 1, max_tiefe, budget, out)?;
            }
        }
        Ok(())
    }

    fn sammle_dateien(&self, dir: &Pfad, out: &mut Vec<Pfad>) -> Result<()> {
        let m = self.manifest_lesen(&self.abs(dir))?;
        for (name, meta) in &m.kinder {
            let kp = dir.verketten(name)?;
            match meta.art {
                Art::Datei => out.push(kp),
                Art::Ordner => self.sammle_dateien(&kp, out)?,
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------------ intern

    /// Absoluter Plattenpfad eines Knotens. Da [`Pfad`] `..`/absolut ausschließt,
    /// bleibt das Ergebnis stets unter der Wurzel (Confinement per Konstruktion).
    fn abs(&self, pfad: &Pfad) -> PathBuf {
        let mut p = self.wurzel.clone();
        for seg in pfad.segmente() {
            p.push(seg);
        }
        p
    }

    /// Schreibt eine Datei und trägt sie ins Eltern-Manifest ein. Erwartet, dass
    /// der Aufrufer die Schreib-Sperre hält.
    fn schreibe_datei(
        &self,
        pfad: &Pfad,
        beschreibung: String,
        unbeschrieben: bool,
        quelle: Option<String>,
        inhalt: &[u8],
    ) -> Result<Knoten> {
        let name = pfad
            .name()
            .ok_or_else(|| SchemeError::UngueltigerPfad("die Wurzel ist keine Datei".into()))?;
        let elter = pfad.elter().unwrap();
        self.ordnerkette_sicherstellen(&elter)?;
        let abs = self.abs(pfad);
        if abs.is_dir() {
            return Err(SchemeError::ArtKonflikt {
                pfad: pfad.to_string(),
                erwartet: "datei",
                vorhanden: "ordner",
            });
        }
        self.atomar_schreiben(&abs, inhalt)?;

        let elter_abs = self.abs(&elter);
        let mut m = self.manifest_lesen(&elter_abs)?;
        let jetzt = jetzt();
        let (erstellt, quelle) = match m.kinder.get(name) {
            Some(e) => (e.erstellt, quelle.or_else(|| e.quelle.clone())),
            None => (jetzt, quelle),
        };
        let meta = Metadaten {
            art: Art::Datei,
            beschreibung,
            unbeschrieben,
            mime: mime_aus_name(name),
            groesse: Some(inhalt.len() as u64),
            quelle,
            erstellt,
            geaendert: jetzt,
        };
        m.kinder.insert(name.to_string(), meta.clone());
        self.manifest_schreiben(&elter_abs, &m)?;
        Ok(Knoten {
            pfad: pfad.clone(),
            metadaten: meta,
            inhalt: None,
        })
    }

    /// Stellt sicher, dass jeder Ordner entlang `dir` existiert und in seinem
    /// Eltern-Manifest als Ordner verzeichnet ist (neu angelegte mit
    /// Platzhalter-Beschreibung + `unbeschrieben`). Erwartet die Schreib-Sperre.
    fn ordnerkette_sicherstellen(&self, dir: &Pfad) -> Result<()> {
        let mut akk = Pfad::wurzel();
        for seg in dir.segmente() {
            let kind = akk.verketten(seg)?;
            let kind_abs = self.abs(&kind);
            if kind_abs.is_file() {
                return Err(SchemeError::ArtKonflikt {
                    pfad: kind.to_string(),
                    erwartet: "ordner",
                    vorhanden: "datei",
                });
            }
            if !kind_abs.exists() {
                fs::create_dir_all(&kind_abs)?;
            }
            let akk_abs = self.abs(&akk);
            let mut m = self.manifest_lesen(&akk_abs)?;
            if !m.kinder.contains_key(seg) {
                let jetzt = jetzt();
                m.kinder.insert(
                    seg.to_string(),
                    Metadaten {
                        art: Art::Ordner,
                        beschreibung: format!("(unbeschrieben – automatisch angelegter Ordner: {seg})"),
                        unbeschrieben: true,
                        mime: None,
                        groesse: None,
                        quelle: None,
                        erstellt: jetzt,
                        geaendert: jetzt,
                    },
                );
                self.manifest_schreiben(&akk_abs, &m)?;
            }
            akk = kind;
        }
        Ok(())
    }

    fn manifest_lesen(&self, dir: &Path) -> Result<Manifest> {
        let mp = manifest_pfad(dir);
        match fs::read(&mp) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| SchemeError::ManifestBeschaedigt(format!("{}: {e}", mp.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Manifest::leer()),
            Err(e) => Err(e.into()),
        }
    }

    fn manifest_schreiben(&self, dir: &Path, m: &Manifest) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(m)
            .map_err(|e| SchemeError::ManifestBeschaedigt(e.to_string()))?;
        self.atomar_schreiben(&manifest_pfad(dir), &bytes)
    }

    /// Atomarer Schreibvorgang: Temp-Datei im selben Verzeichnis, `fsync`,
    /// dann `rename` über das Ziel — Leser sehen nie eine halbe Datei. Sicher,
    /// weil der Writer serialisiert ist (fester Temp-Name je Ziel).
    fn atomar_schreiben(&self, ziel: &Path, bytes: &[u8]) -> Result<()> {
        let dir = ziel
            .parent()
            .expect("Zielpfad hat ein Elternverzeichnis");
        let fname = ziel
            .file_name()
            .expect("Zielpfad hat einen Namen")
            .to_string_lossy();
        let tmp = dir.join(format!(".{fname}.scheme-tmp"));
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, ziel)?;
        // Best-effort: das Umbenennen im Elternverzeichnis durabel machen (ein
        // Verzeichnis-`fsync`), damit der neue Verzeichniseintrag einen Absturz
        // überlebt. Auf Plattformen, die kein Verzeichnis-`fsync` erlauben, ist der
        // Fehler folgenlos.
        if let Ok(d) = fs::File::open(dir) {
            let _ = d.sync_all();
        }
        Ok(())
    }
}

fn manifest_pfad(dir: &Path) -> PathBuf {
    dir.join(MANIFEST_NAME)
}

fn jetzt() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn stub_text(name: &str) -> String {
    format!("(unbeschrieben – automatisch abgelegt: {name})")
}

/// Kleine Hex-Kodierung der ersten 8 Bytes (16 Hex-Zeichen) — deterministischer
/// Name für den `eingang`-Fallback.
fn hex8(b: &[u8]) -> String {
    let mut s = String::with_capacity(16);
    for x in b.iter().take(8) {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

/// Leitet einen MIME-Typ aus der Dateiendung ab (rein mechanisch; `None`, wenn
/// unbekannt). scheme deutet den Inhalt nicht — nur den Namen.
fn mime_aus_name(name: &str) -> Option<String> {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase())?;
    let m = match ext.as_str() {
        "txt" | "md" => "text/plain",
        "json" => "application/json",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "html" | "htm" => "text/html",
        "csv" => "text/csv",
        "bin" => "application/octet-stream",
        _ => return None,
    };
    Some(m.to_string())
}
