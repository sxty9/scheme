//! Nebenläufigkeits-Tests der **Atomare-Zugriffe**-Zusage (§6.1): ein Leser
//! beobachtet **nie** einen Zwischenzustand einer mehrschrittigen Mutation.
//!
//! Beide Tests teilen **einen** [`Bestand`] über mehrere Threads und prüfen eine
//! Invariante, die nur das prozess-interne Lese-Schreib-Schloss (§6.1) garantiert.
//! Ohne das Schloss verschränken sich die Datei- und Manifest-Schritte eines
//! Schreibers mit den Lese-Schritten eines Lesers und liefern einen **zerrissenen**
//! Zustand; mit dem Schloss ist jeder Lese- wie Schreibvorgang atomar.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use scheme_core::{Bestand, Beschreibung, Pfad};
use tempfile::TempDir;

/// `knoten(…, true)` liest Metadaten (Manifest, Größe) und Inhalt (Datei) — ohne
/// Schloss zwei getrennte Zeitpunkte. Ein nebenläufiges `aktualisieren` schreibt
/// erst die Datei, dann das Manifest; verschränkt ergäbe das `groesse ≠ Inhalt`.
/// Mit dem Schloss (§6.1) ist der Knoten stets in sich konsistent.
#[test]
fn knoten_lesen_ist_atomar_unter_nebenlaeufigen_updates() {
    let td = TempDir::new().unwrap();
    let b = Arc::new(Bestand::oeffnen(td.path()).unwrap());
    let p = Pfad::parse("raum/dok.txt").unwrap();

    // Zwei deutlich unterschiedlich lange Zustände, damit eine Verschränkung sofort
    // als Längen-Mismatch auffiele. Der Anfangszustand ist bewusst `kurz`, damit der
    // Inhalt zu **jedem** Zeitpunkt genau eine der beiden kanonischen Längen hat
    // (auch bevor der Schreiber die erste Aktualisierung ausführt).
    let kurz = vec![b'k'; 4];
    let lang = vec![b'l'; 8192];
    b.ablegen(&p, Beschreibung::neu("das Dokument").unwrap(), &kurz)
        .unwrap();
    let fertig = Arc::new(AtomicBool::new(false));

    let schreiber = {
        let (b, p, fertig) = (Arc::clone(&b), p.clone(), Arc::clone(&fertig));
        let (kurz, lang) = (kurz.clone(), lang.clone());
        thread::spawn(move || {
            for i in 0..600 {
                let inhalt = if i % 2 == 0 { &kurz } else { &lang };
                b.aktualisieren(&p, inhalt).unwrap();
            }
            fertig.store(true, Ordering::Release);
        })
    };

    // Vier Leser laufen die ganze Schreiber-Lebenszeit über und prüfen dieselbe
    // Invariante: die gemeldete Größe stimmt exakt mit der Länge des mitgelieferten
    // Inhalts überein, und der Inhalt ist genau einer der beiden geschriebenen
    // Zustände (nie eine Mischung).
    let leser: Vec<_> = (0..4)
        .map(|_| {
            let (b, p, fertig) = (Arc::clone(&b), p.clone(), Arc::clone(&fertig));
            thread::spawn(move || {
                while !fertig.load(Ordering::Acquire) {
                    let k = b.knoten(&p, true).unwrap().expect("Knoten ist vorhanden");
                    let groesse = k.metadaten.groesse.unwrap() as usize;
                    let len = k.inhalt.as_ref().unwrap().len();
                    assert_eq!(groesse, len, "zerrissener Knoten: Größe {groesse} ≠ Inhalt {len}");
                    assert!(len == 4 || len == 8192, "Mischzustand: unerwartete Länge {len}");
                }
            })
        })
        .collect();

    schreiber.join().unwrap();
    for l in leser {
        l.join().unwrap();
    }
}

/// `auflisten` läuft über mehrere Verzeichnis-Manifeste. Ein nebenläufiges
/// `verschieben` über eine Verzeichnisgrenze aktualisiert **zwei** Manifeste
/// (Quelle entfernen, Ziel eintragen). Ohne Schloss könnte `auflisten` die eine
/// Datei an **beiden** oder an **keinem** Pfad sehen. Mit dem Schloss (§6.1) ist
/// die Auflistung ein atomarer Schnappschuss: genau **eine** Datei, an genau einem
/// der beiden Pfade.
#[test]
fn auflisten_ist_atomarer_schnappschuss_unter_nebenlaeufigem_verschieben() {
    let td = TempDir::new().unwrap();
    let b = Arc::new(Bestand::oeffnen(td.path()).unwrap());
    let a = Pfad::parse("a/x.txt").unwrap();
    let c = Pfad::parse("c/x.txt").unwrap();
    b.ablegen(&a, Beschreibung::neu("die eine Datei").unwrap(), b"NUTZLAST")
        .unwrap();

    let fertig = Arc::new(AtomicBool::new(false));

    let schreiber = {
        let (b, a, c, fertig) = (Arc::clone(&b), a.clone(), c.clone(), Arc::clone(&fertig));
        thread::spawn(move || {
            for i in 0..600 {
                // Die Datei pendelt zwischen a/ und c/ — sie liegt in jedem
                // vollständigen Zustand an genau einem der beiden Pfade.
                if i % 2 == 0 {
                    b.verschieben(&a, &c).unwrap();
                } else {
                    b.verschieben(&c, &a).unwrap();
                }
            }
            fertig.store(true, Ordering::Release);
        })
    };

    let leser: Vec<_> = (0..4)
        .map(|_| {
            let (b, fertig) = (Arc::clone(&b), Arc::clone(&fertig));
            thread::spawn(move || {
                while !fertig.load(Ordering::Acquire) {
                    let alle = b.auflisten(&Pfad::wurzel()).unwrap();
                    assert_eq!(
                        alle.len(),
                        1,
                        "kein atomarer Schnappschuss: {:?}",
                        alle.iter().map(|p| p.als_str()).collect::<Vec<_>>()
                    );
                    let nur = alle[0].als_str();
                    assert!(
                        nur == "a/x.txt" || nur == "c/x.txt",
                        "unerwarteter Pfad im Schnappschuss: {nur}"
                    );
                }
            })
        })
        .collect();

    schreiber.join().unwrap();
    for l in leser {
        l.join().unwrap();
    }
}
