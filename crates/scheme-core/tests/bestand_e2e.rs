//! End-to-End-Tests des [`Bestand`] über echte Temp-Verzeichnisse (§5/§6/§7).
//!
//! Prüft: Roundtrip strukturiert ablegen → lesen; die **erzwungene**
//! Pflicht-Beschreibung (§4, Negativtest); Aktualisieren in-place; Verschieben
//! (re-key, Inhalt + Beschreibung bleiben); Löschen (idempotent); Auflisten;
//! den `eingang`-Fallback des Basis-`ablegen_roh`; und dass die klare Struktur
//! als echte Verzeichnisse auf der Platte liegt.

use scheme_core::model::Art;
use scheme_core::{Bestand, Beschreibung, Pfad, SchemeError};
use tempfile::TempDir;

fn bestand() -> (TempDir, Bestand) {
    let td = TempDir::new().unwrap();
    let b = Bestand::oeffnen(td.path()).unwrap();
    (td, b)
}

fn pfad(s: &str) -> Pfad {
    Pfad::parse(s).unwrap()
}

fn beschr(s: &str) -> Beschreibung {
    Beschreibung::neu(s).unwrap()
}

#[test]
fn roundtrip_strukturiert_ablegen_und_lesen() {
    let (_td, b) = bestand();
    let p = pfad("praesentationsraum/hardware/computer/bedienungsanleitung.pdf");
    b.ablegen(&p, beschr("Bedienungsanleitung des Präsentations-Computers"), b"PDF-BYTES")
        .unwrap();

    assert_eq!(b.lesen(&p).unwrap().as_deref(), Some(&b"PDF-BYTES"[..]));
    let k = b.knoten(&p, false).unwrap().unwrap();
    assert_eq!(k.art(), Art::Datei);
    assert_eq!(k.beschreibung(), "Bedienungsanleitung des Präsentations-Computers");
    assert!(!k.metadaten.unbeschrieben);
    assert_eq!(k.metadaten.groesse, Some(9));
    assert_eq!(k.metadaten.mime.as_deref(), Some("application/pdf"));
}

#[test]
fn struktur_liegt_als_echte_verzeichnisse_auf_der_platte() {
    let (_td, b) = bestand();
    let p = pfad("praesentationsraum/hardware/computer/anleitung.txt");
    b.ablegen(&p, beschr("Anleitung"), b"hallo").unwrap();

    let wurzel = b.wurzel();
    assert!(wurzel.join("praesentationsraum/hardware/computer").is_dir());
    assert!(wurzel.join("praesentationsraum/hardware/computer/anleitung.txt").is_file());
    // Jedes Verzeichnis trägt sein Manifest — der Bestand ist „komplett lesbar".
    assert!(wurzel.join("praesentationsraum/hardware/computer/.scheme.json").is_file());
    // Der Datei-Inhalt liegt unverändert als echte Datei vor.
    assert_eq!(
        std::fs::read(wurzel.join("praesentationsraum/hardware/computer/anleitung.txt")).unwrap(),
        b"hallo"
    );
}

#[test]
fn write_rejects_empty_description() {
    // §4: die Pflicht-Beschreibung ist maschinell erzwungen.
    assert!(matches!(
        Beschreibung::neu(""),
        Err(SchemeError::BeschreibungFehlt)
    ));
    assert!(matches!(
        Beschreibung::neu("   \t\n "),
        Err(SchemeError::BeschreibungFehlt)
    ));
}

#[test]
fn aktualisieren_ersetzt_inhalt_erhaelt_beschreibung() {
    let (_td, b) = bestand();
    let p = pfad("a/b/c.txt");
    b.ablegen(&p, beschr("das C-Dokument"), b"eins").unwrap();
    b.aktualisieren(&p, b"zwei-lang").unwrap();

    assert_eq!(b.lesen(&p).unwrap().as_deref(), Some(&b"zwei-lang"[..]));
    let k = b.knoten(&p, false).unwrap().unwrap();
    assert_eq!(k.beschreibung(), "das C-Dokument");
    assert_eq!(k.metadaten.groesse, Some(9));
}

#[test]
fn aktualisieren_fehlt_ist_nicht_gefunden() {
    let (_td, b) = bestand();
    assert!(matches!(
        b.aktualisieren(&pfad("gibt/es/nicht.txt"), b"x"),
        Err(SchemeError::NichtGefunden(_))
    ));
}

#[test]
fn move_preserves_content_and_description() {
    let (_td, b) = bestand();
    let von = pfad("eingang/rohdatei.txt");
    let nach = pfad("archiv/2026/sortiert.txt");
    b.ablegen(&von, beschr("wichtige Rohdatei"), b"NUTZLAST").unwrap();

    b.verschieben(&von, &nach).unwrap();

    assert_eq!(b.lesen(&von).unwrap(), None, "Quelle ist weg");
    assert_eq!(b.lesen(&nach).unwrap().as_deref(), Some(&b"NUTZLAST"[..]));
    let k = b.knoten(&nach, false).unwrap().unwrap();
    assert_eq!(k.beschreibung(), "wichtige Rohdatei", "Beschreibung wandert mit");
}

#[test]
fn verschieben_auf_belegten_pfad_wird_abgelehnt() {
    let (_td, b) = bestand();
    b.ablegen(&pfad("a/x.txt"), beschr("x"), b"1").unwrap();
    b.ablegen(&pfad("a/y.txt"), beschr("y"), b"2").unwrap();
    assert!(matches!(
        b.verschieben(&pfad("a/x.txt"), &pfad("a/y.txt")),
        Err(SchemeError::BereitsVorhanden(_))
    ));
}

#[test]
fn loeschen_ist_idempotent() {
    let (_td, b) = bestand();
    let p = pfad("a/b/c.txt");
    b.ablegen(&p, beschr("c"), b"inhalt").unwrap();
    assert!(b.loeschen(&p).unwrap(), "erstes Löschen entfernt");
    assert!(!b.loeschen(&p).unwrap(), "zweites Löschen ist no-op");
    assert_eq!(b.lesen(&p).unwrap(), None);
}

#[test]
fn loeschen_ordner_entfernt_teilbaum() {
    let (_td, b) = bestand();
    b.ablegen(&pfad("raum/a.txt"), beschr("a"), b"1").unwrap();
    b.ablegen(&pfad("raum/tief/b.txt"), beschr("b"), b"2").unwrap();
    assert!(b.loeschen(&pfad("raum")).unwrap());
    assert_eq!(b.lesen(&pfad("raum/a.txt")).unwrap(), None);
    assert_eq!(b.lesen(&pfad("raum/tief/b.txt")).unwrap(), None);
    assert!(!b.wurzel().join("raum").exists());
}

#[test]
fn auflisten_enumeriert_dateien_sortiert_unter_prefix() {
    let (_td, b) = bestand();
    b.ablegen(&pfad("raum/hardware/computer/anleitung.txt"), beschr("c"), b"1").unwrap();
    b.ablegen(&pfad("raum/hardware/beamer/anleitung.txt"), beschr("beamer"), b"2").unwrap();
    b.ablegen(&pfad("raum/moebel/stuhl.txt"), beschr("stuhl"), b"3").unwrap();

    let unter_hardware = b.auflisten(&pfad("raum/hardware")).unwrap();
    let als_str: Vec<&str> = unter_hardware.iter().map(|p| p.als_str()).collect();
    assert_eq!(
        als_str,
        vec![
            "raum/hardware/beamer/anleitung.txt",
            "raum/hardware/computer/anleitung.txt",
        ]
    );

    let alle = b.auflisten(&Pfad::wurzel()).unwrap();
    assert_eq!(alle.len(), 3);
}

#[test]
fn eingang_fallback_ohne_pfad_ist_deterministisch_und_unbeschrieben() {
    let (_td, b) = bestand();
    let k1 = b.ablegen_roh(None, b"lose bytes").unwrap();
    let k2 = b.ablegen_roh(None, b"lose bytes").unwrap();
    assert_eq!(k1.pfad, k2.pfad, "gleicher Inhalt ⇒ gleicher eingang-Pfad");
    assert!(k1.pfad.als_str().starts_with("eingang/"));
    assert!(k1.metadaten.unbeschrieben, "eingang-Fallback ist unbeschrieben (§6.4)");
    assert_eq!(b.lesen(&k1.pfad).unwrap().as_deref(), Some(&b"lose bytes"[..]));
}

#[test]
fn ablegen_roh_mit_pfad_erhaelt_bestehende_beschreibung() {
    let (_td, b) = bestand();
    let p = pfad("a/dok.txt");
    b.ablegen(&p, beschr("die klare Beschreibung"), b"v1").unwrap();
    // Basis-Put (roh) über denselben Pfad: Inhalt neu, Beschreibung bleibt.
    b.ablegen_roh(Some(&p), b"v2-neu").unwrap();
    let k = b.knoten(&p, false).unwrap().unwrap();
    assert_eq!(k.beschreibung(), "die klare Beschreibung");
    assert!(!k.metadaten.unbeschrieben);
    assert_eq!(b.lesen(&p).unwrap().as_deref(), Some(&b"v2-neu"[..]));
}

#[test]
fn beschreiben_setzt_beschreibung_und_loescht_unbeschrieben_flag() {
    let (_td, b) = bestand();
    let k = b.ablegen_roh(None, b"irgendwas").unwrap();
    assert!(k.metadaten.unbeschrieben);
    let k2 = b
        .beschreiben(&k.pfad, beschr("jetzt klar eingeordnet"))
        .unwrap();
    assert_eq!(k2.beschreibung(), "jetzt klar eingeordnet");
    assert!(!k2.metadaten.unbeschrieben);
}

#[test]
fn traversieren_ist_beschraenkt_und_deterministisch() {
    let (_td, b) = bestand();
    b.ablegen(&pfad("raum/hardware/computer.txt"), beschr("c"), b"1").unwrap();
    b.ablegen(&pfad("raum/hardware/beamer.txt"), beschr("b"), b"2").unwrap();
    b.ablegen(&pfad("raum/moebel/stuhl.txt"), beschr("s"), b"3").unwrap();

    let knoten = b.traversieren(&pfad("raum"), 8, 1000).unwrap();
    let pfade: Vec<&str> = knoten.iter().map(|k| k.pfad.als_str()).collect();
    assert_eq!(
        pfade,
        vec![
            "raum/hardware",
            "raum/hardware/beamer.txt",
            "raum/hardware/computer.txt",
            "raum/moebel",
            "raum/moebel/stuhl.txt",
        ]
    );

    // Knoten-Budget begrenzt die Ausgabe.
    let begrenzt = b.traversieren(&pfad("raum"), 8, 2).unwrap();
    assert_eq!(begrenzt.len(), 2);
}
