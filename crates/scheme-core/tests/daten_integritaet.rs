//! Tests der **Daten-Integrität** — die beiden tragenden Axiome des Substrats:
//!
//! - **Atomare Zugriffe** (§6.1): alle Zugriffe laufen durch *ein* Zugriffs-Schloss
//!   — Mutationen exklusiv (der eine serialisierte Writer), mehr-Datei-überspannende
//!   Leser geteilt. Jeder Datei-/Manifest-Schreibvorgang ist zudem für sich atomar
//!   (Temp-Datei + `rename`). So sieht ein gleichzeitiger Leser **nie** einen halb
//!   geschriebenen Zustand — weder eine zerrissene *einzelne* Datei noch den
//!   Zwischenzustand einer *mehrschrittigen* Mutation (Verschieben über
//!   Verzeichnisgrenzen).
//! - **Passiver Speicher** (§1.4): scheme **wertet/sortiert nicht** — es ordnet
//!   Ausgaben allein deterministisch nach **Pfad**, unabhängig vom Wert.
//!
//! Diese Zusagen waren bislang nur in Prosa formuliert. Hier werden sie unter echter
//! Nebenläufigkeit über echte Temp-Verzeichnisse als Regressionsschutz eingefroren.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use scheme_core::{Art, Bestand, Beschreibung, Pfad, SchemeIndex};
use tempfile::TempDir;

fn p(s: &str) -> Pfad {
    Pfad::parse(s).unwrap()
}
fn d(s: &str) -> Beschreibung {
    Beschreibung::neu(s).unwrap()
}

/// §6.1: **ein serialisierter Writer** — viele nebenläufige Schreiber, die alle in
/// **dasselbe** Eltern-Manifest (den heiß umkämpften geteilten Zustand) schreiben,
/// dürfen sich nie gegenseitig überschreiben (verlorener Eintrag) oder das Manifest
/// zerreißen. Am Ende ist jede Datei intakt, das Manifest vollständig, und der
/// Index-Neubau (§8) — eine reine Projektion des Baums — zählt exakt jede Ablage.
#[test]
fn nebenlaeufige_schreiber_serialisiert_ohne_korruption() {
    let td = TempDir::new().unwrap();
    let b = Arc::new(Bestand::oeffnen(td.path()).unwrap());
    let threads = 8usize;
    let pro_thread = 50usize;

    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let b = Arc::clone(&b);
            thread::spawn(move || {
                for i in 0..pro_thread {
                    let pfad = p(&format!("gemeinsam/datei-{t:02}-{i:02}.txt"));
                    let inhalt = format!("t{t}-i{i}");
                    b.ablegen(&pfad, d("nebenläufig abgelegt"), inhalt.as_bytes())
                        .unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }

    // Jede einzelne Datei ist intakt und mit korrektem Inhalt lesbar.
    for t in 0..threads {
        for i in 0..pro_thread {
            let pfad = p(&format!("gemeinsam/datei-{t:02}-{i:02}.txt"));
            let erwartet = format!("t{t}-i{i}");
            assert_eq!(
                b.lesen(&pfad).unwrap().as_deref(),
                Some(erwartet.as_bytes()),
                "verlorener/zerrissener Schreibvorgang bei {pfad}"
            );
        }
    }

    // Das gemeinsame Manifest ist unversehrt: genau threads*pro_thread Kinder,
    // alle als Datei — kein verlorener Eintrag, kein Duplikat.
    let kinder = b.kinder(&p("gemeinsam")).unwrap();
    assert_eq!(
        kinder.len(),
        threads * pro_thread,
        "Kinder gingen verloren oder duplizierten (Manifest-Wettlauf)"
    );
    assert!(kinder.iter().all(|k| k.art() == Art::Datei));
    assert_eq!(b.auflisten(&Pfad::wurzel()).unwrap().len(), threads * pro_thread);

    // Der Index (§8) ist ein reines, neu-baubares Derivat des Baums: er muss genau
    // die serialisierten Knoten sehen — threads*pro_thread Dateien + den einen
    // gemeinsamen Ordner.
    let idx_dir = TempDir::new().unwrap();
    let mut idx = SchemeIndex::oeffnen(idx_dir.path().join("index.redb")).unwrap();
    idx.neu_bauen_aus(&b).unwrap();
    assert_eq!(idx.anzahl().unwrap(), threads * pro_thread + 1);
}

/// §6.1 / „ohne beobachtbaren Zwischenzustand": ein Leser, der **während** eines
/// laufenden `aktualisieren` liest, sieht **immer** genau eine vollständige Version —
/// nie eine Mischung oder eine abgeschnittene Datei. Der Schreiber wechselt zwischen
/// zwei Mustern **unterschiedlicher Länge**; jeder gelesene Wert muss exakt eines der
/// beiden sein. Das ist der Kern der atomaren Ablage (Temp-Datei + `rename`): der
/// Leser öffnet stets einen kohärenten Inode, nie eine in-place halb überschriebene
/// Datei.
#[test]
fn leser_sieht_nie_zerrissenen_wert_beim_aktualisieren() {
    let td = TempDir::new().unwrap();
    let b = Arc::new(Bestand::oeffnen(td.path()).unwrap());
    let pfad = p("wettlauf/wert.bin");
    let klein = vec![b'A'; 512];
    let gross = vec![b'B'; 4096];
    b.ablegen(&pfad, d("umkämpfter Wert"), &klein).unwrap();

    let stop = Arc::new(AtomicBool::new(false));
    let schreiber = {
        let b = Arc::clone(&b);
        let pfad = pfad.clone();
        let klein = klein.clone();
        let gross = gross.clone();
        let stop = Arc::clone(&stop);
        thread::spawn(move || {
            let mut runden = 0u64;
            while !stop.load(Ordering::Relaxed) {
                let inhalt = if runden.is_multiple_of(2) { &klein } else { &gross };
                b.aktualisieren(&pfad, inhalt).unwrap();
                runden += 1;
            }
            runden
        })
    };

    // Der Leser prüft die Atomaritäts-Invariante viele Male.
    for _ in 0..20_000 {
        let v = b.lesen(&pfad).unwrap().expect("der Wert ist stets vorhanden");
        let voll_klein = v.len() == 512 && v.iter().all(|&x| x == b'A');
        let voll_gross = v.len() == 4096 && v.iter().all(|&x| x == b'B');
        assert!(
            voll_klein || voll_gross,
            "zerrissener Wert gelesen: len={}, erstes Byte={:?}",
            v.len(),
            v.first()
        );
    }

    stop.store(true, Ordering::Relaxed);
    let runden = schreiber.join().unwrap();
    assert!(runden > 0, "der Schreiber lief nicht — Test beweist nichts");
}

/// §6.1 / „ohne beobachtbaren Zwischenzustand" — die **mehrschrittige** Mutation.
/// Ein `verschieben` über Verzeichnisgrenzen ist intern **kein** einzelner
/// atomarer Schritt: es ist ein `rename` plus **zwei** getrennte Manifest-Schreib-
/// vorgänge (Quelle raus, Ziel rein). Ein Leser, der **nicht** gegen den Schreiber
/// koordiniert ist, kann diesen Vorgang auf halbem Weg beobachten — etwa ein
/// Manifest, das die Datei noch listet, während sie auf der Platte bereits am neuen
/// Ort liegt. Dieser Test hält die Zusage *jeder Lesezugriff ist als Ganzes atomar*
/// unter echter Nebenläufigkeit fest.
///
/// Geprüft werden ausschließlich **Einzelaufruf-Invarianten** (zwei getrennte
/// Aufrufe zu vergleichen wäre ein Test-eigener Wettlauf, der auch mit der Sperre
/// bräche):
/// - `knoten(pfad, mit_inhalt=true)` liest Manifest **und** Inhalt unter *einer*
///   Lese-Sperre: sagt das Ergebnis „Datei da", MUSS der Inhalt vollständig
///   vorliegen — nie `None` (der Zerriss-Zustand, den ein sperrenloser Leser
///   zwischen `rename` und Manifest-Aktualisierung sähe).
/// - `auflisten` zeigt die Datei an **genau einem** Ort (nie null, nie zwei).
#[test]
fn leser_sieht_verschieben_nie_auf_halbem_weg() {
    let td = TempDir::new().unwrap();
    let b = Arc::new(Bestand::oeffnen(td.path()).unwrap());
    let a_pfad = p("a/x.txt");
    let b_pfad = p("b/x.txt");
    let nutzlast = b"NUTZLAST-die-vollstaendig-vorliegen-muss".to_vec();
    b.ablegen(&a_pfad, d("umkämpfte Datei, wandert zwischen a/ und b/"), &nutzlast)
        .unwrap();

    // Der **Leser** läuft nebenläufig und prüft die Atomaritäts-Invariante Runde um
    // Runde, bis der Schreiber (unten, begrenzt) das Ende signalisiert.
    let stop = Arc::new(AtomicBool::new(false));
    let leser = {
        let b = Arc::clone(&b);
        let a_pfad = a_pfad.clone();
        let b_pfad = b_pfad.clone();
        let nutzlast = nutzlast.clone();
        let stop = Arc::clone(&stop);
        thread::spawn(move || {
            let mut runden = 0u64;
            while !stop.load(Ordering::Relaxed) {
                for pfad in [&a_pfad, &b_pfad] {
                    if let Some(k) = b.knoten(pfad, true).unwrap() {
                        assert_eq!(k.art(), Art::Datei);
                        assert_eq!(
                            k.inhalt.as_deref(),
                            Some(&nutzlast[..]),
                            "Knoten an {pfad}: Manifest listet die Datei, aber der Inhalt fehlt/ist \
                             zerrissen — der Zwischenzustand eines Verschiebens wurde beobachtet"
                        );
                    }
                }
                // Die Datei existiert stets an genau einem Ort — die Auflistung (viele
                // Manifeste) darf nie null oder zwei Vorkommen zeigen.
                let liste = b.auflisten(&Pfad::wurzel()).unwrap();
                assert_eq!(
                    liste.len(),
                    1,
                    "Auflistung zeigt die Datei nicht an genau einem Ort: {:?}",
                    liste.iter().map(|q| q.als_str().to_string()).collect::<Vec<_>>()
                );
                runden += 1;
                // Dem Schreiber ein Fortschritts-Fenster geben (sonst könnte der Leser
                // den fsync-schweren Verschieber aushungern und ihn verzögern).
                thread::yield_now();
            }
            runden
        })
    };

    // Der **Schreiber** (dieser Thread) macht eine **begrenzte** Zahl Verschiebungen
    // über die Verzeichnisgrenze — beschränkte, last-unabhängige Laufzeit — und
    // signalisiert dann das Ende. So besucht die Datei viele Male beide Orte, während
    // der Leser nebenläufig prüft.
    for runde in 0..300u64 {
        if runde.is_multiple_of(2) {
            b.verschieben(&a_pfad, &b_pfad).unwrap();
        } else {
            b.verschieben(&b_pfad, &a_pfad).unwrap();
        }
    }
    stop.store(true, Ordering::Relaxed);

    let leser_runden = leser.join().unwrap();
    assert!(
        leser_runden > 0,
        "der Leser lief nicht neben dem Verschieben — Test beweist nichts"
    );
}

/// §6.1: der atomare Schreibvorgang (Temp-Datei + `rename`) darf nach Abschluss
/// **keine** `.scheme-tmp`-Reste hinterlassen — sonst leckte der Zwischenzustand auf
/// die Platte und würde für einen Menschen sichtbar. Nach dem vollen Mutations-
/// Reigen (ablegen/aktualisieren/verschieben/beschreiben/löschen) ist der Baum
/// temp-frei.
#[test]
fn atomares_schreiben_hinterlaesst_keinen_temp_muell() {
    let td = TempDir::new().unwrap();
    let b = Bestand::oeffnen(td.path()).unwrap();
    b.ablegen(&p("a/b/c.txt"), d("c"), b"eins").unwrap();
    b.aktualisieren(&p("a/b/c.txt"), b"zwei").unwrap();
    b.ablegen(&p("a/b/e.txt"), d("e"), b"drei").unwrap();
    b.verschieben(&p("a/b/e.txt"), &p("a/f/e.txt")).unwrap();
    b.beschreiben(&p("a/b/c.txt"), d("neu beschrieben")).unwrap();
    b.loeschen(&p("a/f/e.txt")).unwrap();

    let mut reste = Vec::new();
    fuer_jede_datei(td.path(), &mut |pfad| {
        if pfad
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".scheme-tmp"))
        {
            reste.push(pfad.to_path_buf());
        }
    });
    assert!(reste.is_empty(), "Temp-Reste auf der Platte geblieben: {reste:?}");
}

/// §1.4: scheme ist ein **passiver Speicher** — es sortiert **nicht nach Wert**
/// (Größe, Zeit, Inhalt), sondern ordnet allein deterministisch nach **Pfad**. Die
/// Dateien werden in einer Reihenfolge und mit Größen angelegt, die jeder wert-
/// basierten Sortierung widersprechen; die Ausgabe muss dennoch rein nach Pfad
/// geordnet sein.
#[test]
fn ordnung_folgt_pfad_nicht_groesse_oder_zeit() {
    let td = TempDir::new().unwrap();
    let b = Bestand::oeffnen(td.path()).unwrap();
    // Anlege-Reihenfolge z→a→m (nicht alphabetisch); Größe fällt mit dem Alphabet,
    // sodass „nach Größe" die Pfad-Ordnung gerade umkehrte.
    b.ablegen(&p("ordner/z.txt"), d("z"), &vec![b'z'; 3000]).unwrap();
    b.ablegen(&p("ordner/a.txt"), d("a"), &vec![b'a'; 2000]).unwrap();
    b.ablegen(&p("ordner/m.txt"), d("m"), &vec![b'm'; 1000]).unwrap();

    let kinder: Vec<String> = b
        .kinder(&p("ordner"))
        .unwrap()
        .iter()
        .map(|k| k.name().unwrap().to_string())
        .collect();
    assert_eq!(
        kinder,
        vec!["a.txt", "m.txt", "z.txt"],
        "Kinder nicht rein nach Pfad geordnet (Wert-Sortierung?)"
    );

    let liste: Vec<String> = b
        .auflisten(&p("ordner"))
        .unwrap()
        .iter()
        .map(|q| q.als_str().to_string())
        .collect();
    assert_eq!(
        liste,
        vec!["ordner/a.txt", "ordner/m.txt", "ordner/z.txt"],
        "Auflisten nicht rein nach Pfad geordnet"
    );
}

/// Läuft `f` für jede reguläre Datei unterhalb `dir` (rekursiv).
fn fuer_jede_datei(dir: &Path, f: &mut impl FnMut(&Path)) {
    for eintrag in std::fs::read_dir(dir).unwrap() {
        let pfad = eintrag.unwrap().path();
        if pfad.is_dir() {
            fuer_jede_datei(&pfad, f);
        } else {
            f(&pfad);
        }
    }
}
