//! Tests des abgeleiteten Index (§8): der Index ist eine reine Projektion des
//! Baums und wird **wipe-&-rebuild** wieder byte-gleich hergestellt (§8.2).

use scheme_core::{Bestand, Beschreibung, Pfad, SchemeIndex};
use tempfile::TempDir;

fn aufbau() -> (TempDir, Bestand, std::path::PathBuf) {
    let td = TempDir::new().unwrap();
    let b = Bestand::oeffnen(td.path().join("bestand")).unwrap();
    let idx = td.path().join("scheme-index.redb");
    let p = |s: &str| Pfad::parse(s).unwrap();
    let d = |s: &str| Beschreibung::neu(s).unwrap();
    b.ablegen(&p("raum/hardware/computer/anleitung.pdf"), d("Bedienungsanleitung des Computers"), b"A").unwrap();
    b.ablegen(&p("raum/hardware/beamer/anleitung.pdf"), d("Bedienungsanleitung des Beamers"), b"B").unwrap();
    b.ablegen(&p("raum/audio/lautsprecher/handbuch.pdf"), d("Handbuch des Lautsprechersystems"), b"C").unwrap();
    (td, b, idx)
}

#[test]
fn index_findet_metadaten_und_prefix() {
    let (_td, b, idxpfad) = aufbau();
    let mut idx = SchemeIndex::oeffnen(&idxpfad).unwrap();
    idx.neu_bauen_aus(&b).unwrap();

    let p = Pfad::parse("raum/hardware/computer/anleitung.pdf").unwrap();
    let meta = idx.metadaten(&p).unwrap().unwrap();
    assert_eq!(meta.beschreibung, "Bedienungsanleitung des Computers");

    let unter_hardware = idx.unter_prefix(&Pfad::parse("raum/hardware").unwrap()).unwrap();
    // Der Präfix-Knoten selbst + Ordner + Dateien darunter, sortiert
    // (unter_prefix ist reflexiv: „unterhalb bzw. gleich prefix").
    let s: Vec<&str> = unter_hardware.iter().map(|p| p.als_str()).collect();
    assert_eq!(
        s,
        vec![
            "raum/hardware",
            "raum/hardware/beamer",
            "raum/hardware/beamer/anleitung.pdf",
            "raum/hardware/computer",
            "raum/hardware/computer/anleitung.pdf",
        ]
    );
}

#[test]
fn suche_ueber_beschreibungen() {
    let (_td, b, idxpfad) = aufbau();
    let mut idx = SchemeIndex::oeffnen(&idxpfad).unwrap();
    idx.neu_bauen_aus(&b).unwrap();

    let treffer = idx.suche_beschreibung("bedienungsanleitung").unwrap();
    let s: Vec<&str> = treffer.iter().map(|p| p.als_str()).collect();
    assert_eq!(
        s,
        vec![
            "raum/hardware/beamer/anleitung.pdf",
            "raum/hardware/computer/anleitung.pdf",
        ]
    );
}

#[test]
fn wipe_und_rebuild_stellt_gleichen_zustand_her() {
    let (_td, b, idxpfad) = aufbau();
    let mut idx = SchemeIndex::oeffnen(&idxpfad).unwrap();

    idx.neu_bauen_aus(&b).unwrap();
    let vor_anzahl = idx.anzahl().unwrap();
    let vor_liste = idx.unter_prefix(&Pfad::wurzel()).unwrap();
    assert!(vor_anzahl > 0);

    // Erneut bauen = wipe + rebuild aus dem Baum: identischer Zustand (§8.2).
    idx.neu_bauen_aus(&b).unwrap();
    assert_eq!(idx.anzahl().unwrap(), vor_anzahl, "kein Duplizieren beim Neu-Bauen");
    assert_eq!(idx.unter_prefix(&Pfad::wurzel()).unwrap(), vor_liste, "gleiche Projektion");

    // Der Index folgt Baum-Änderungen nach einem Neu-Bau.
    b.loeschen(&Pfad::parse("raum/audio").unwrap()).unwrap();
    idx.neu_bauen_aus(&b).unwrap();
    assert!(idx
        .metadaten(&Pfad::parse("raum/audio/lautsprecher/handbuch.pdf").unwrap())
        .unwrap()
        .is_none());
}
