//! Integrationstest der C-ABI: treibt die `extern "C"`-Symbole genau wie ein
//! C-/cgo-Aufrufer (der Test linkt über die `rlib`-Sicht des Crates). Prüft den
//! Roundtrip, die **erzwungene** Beschreibung, das Puffer-Protokoll (buffer-too-
//! small + Retry), Idempotenz, Fehler-Codes und den Panik-Wächter.

use scheme_ffi::*;
use std::os::raw::c_char;
use std::ptr;
use tempfile::TempDir;

/// Öffnet einen Bestand über die FFI und gibt (TempDir, Handle) zurück.
fn open() -> (TempDir, *mut SchemeHandle) {
    let td = TempDir::new().unwrap();
    let dir = td.path().to_str().unwrap();
    let mut handle: *mut SchemeHandle = ptr::null_mut();
    let st = unsafe {
        scheme_open(
            dir.as_ptr() as *const c_char,
            dir.len(),
            &mut handle as *mut *mut SchemeHandle,
        )
    };
    assert_eq!(st, SchemeStatus::Ok);
    assert!(!handle.is_null());
    (td, handle)
}

fn put_struct(h: *mut SchemeHandle, path: &str, desc: &str, data: &[u8]) -> SchemeStatus {
    unsafe {
        scheme_put_structured(
            h,
            path.as_ptr() as *const c_char,
            path.len(),
            desc.as_ptr() as *const c_char,
            desc.len(),
            data.as_ptr(),
            data.len(),
        )
    }
}

/// Liest über das Puffer-Protokoll: erst mit Kapazität 0 (Längen-Probe), dann mit
/// exakt der benötigten Länge.
fn get(h: *mut SchemeHandle, path: &str) -> (SchemeStatus, Vec<u8>) {
    let mut need: usize = 0;
    let probe = unsafe {
        scheme_get(
            h,
            path.as_ptr() as *const c_char,
            path.len(),
            ptr::null_mut(),
            &mut need as *mut usize,
        )
    };
    if probe == SchemeStatus::NotFound {
        return (probe, Vec::new());
    }
    // Bei nicht-leerem Inhalt: erste Probe meldet BufferTooSmall + benötigte Länge.
    if need == 0 {
        assert_eq!(probe, SchemeStatus::Ok);
        return (SchemeStatus::Ok, Vec::new());
    }
    assert_eq!(probe, SchemeStatus::BufferTooSmall);
    let mut buf = vec![0u8; need];
    let mut cap = need;
    let st = unsafe {
        scheme_get(
            h,
            path.as_ptr() as *const c_char,
            path.len(),
            buf.as_mut_ptr(),
            &mut cap as *mut usize,
        )
    };
    buf.truncate(cap);
    (st, buf)
}

#[test]
fn roundtrip_put_structured_and_get() {
    let (_td, h) = open();
    assert_eq!(
        put_struct(h, "raum/hardware/computer/anleitung.pdf", "Anleitung", b"PDF"),
        SchemeStatus::Ok
    );
    let (st, bytes) = get(h, "raum/hardware/computer/anleitung.pdf");
    assert_eq!(st, SchemeStatus::Ok);
    assert_eq!(bytes, b"PDF");
    assert_eq!(unsafe { scheme_close(h) }, SchemeStatus::Ok);
}

#[test]
fn empty_description_is_rejected() {
    let (_td, h) = open();
    assert_eq!(
        put_struct(h, "a/b.txt", "   ", b"x"),
        SchemeStatus::DescriptionRequired
    );
    assert_eq!(unsafe { scheme_close(h) }, SchemeStatus::Ok);
}

#[test]
fn get_absent_is_not_found() {
    let (_td, h) = open();
    let (st, _) = get(h, "gibt/es/nicht.txt");
    assert_eq!(st, SchemeStatus::NotFound);
    unsafe { scheme_close(h) };
}

#[test]
fn base_put_without_path_lands_in_eingang() {
    let (_td, h) = open();
    let mut path_buf = vec![0u8; 256];
    let mut cap = path_buf.len();
    let st = unsafe {
        scheme_put(
            h,
            ptr::null(),
            0,
            b"lose".as_ptr(),
            4,
            path_buf.as_mut_ptr(),
            &mut cap as *mut usize,
        )
    };
    assert_eq!(st, SchemeStatus::Ok);
    path_buf.truncate(cap);
    let pfad = String::from_utf8(path_buf).unwrap();
    assert!(pfad.starts_with("eingang/"), "gelandet unter: {pfad}");
    unsafe { scheme_close(h) };
}

#[test]
fn update_move_delete_flow() {
    let (_td, h) = open();
    put_struct(h, "a/dok.txt", "Dokument", b"v1");

    // update
    assert_eq!(
        unsafe { scheme_update(h, "a/dok.txt".as_ptr() as *const c_char, 9, b"v2".as_ptr(), 2) },
        SchemeStatus::Ok
    );
    let (_, bytes) = get(h, "a/dok.txt");
    assert_eq!(bytes, b"v2");

    // move
    let (von, nach) = ("a/dok.txt", "b/ziel.txt");
    assert_eq!(
        unsafe {
            scheme_move(
                h,
                von.as_ptr() as *const c_char,
                von.len(),
                nach.as_ptr() as *const c_char,
                nach.len(),
            )
        },
        SchemeStatus::Ok
    );
    assert_eq!(get(h, von).0, SchemeStatus::NotFound);
    assert_eq!(get(h, nach).1, b"v2");

    // delete (idempotent)
    let mut removed: i32 = -1;
    assert_eq!(
        unsafe { scheme_delete(h, nach.as_ptr() as *const c_char, nach.len(), &mut removed) },
        SchemeStatus::Ok
    );
    assert_eq!(removed, 1);
    assert_eq!(
        unsafe { scheme_delete(h, nach.as_ptr() as *const c_char, nach.len(), &mut removed) },
        SchemeStatus::Ok
    );
    assert_eq!(removed, 0, "zweites Löschen ist no-op");
    unsafe { scheme_close(h) };
}

#[test]
fn update_missing_is_not_found() {
    let (_td, h) = open();
    let p = "fehlt.txt";
    assert_eq!(
        unsafe { scheme_update(h, p.as_ptr() as *const c_char, p.len(), b"x".as_ptr(), 1) },
        SchemeStatus::NotFound
    );
    unsafe { scheme_close(h) };
}

#[test]
fn list_returns_newline_separated_paths() {
    let (_td, h) = open();
    put_struct(h, "raum/a.txt", "a", b"1");
    put_struct(h, "raum/tief/b.txt", "b", b"2");

    let mut need: usize = 0;
    let prefix = "raum";
    assert_eq!(
        unsafe {
            scheme_list(
                h,
                prefix.as_ptr() as *const c_char,
                prefix.len(),
                ptr::null_mut(),
                &mut need,
            )
        },
        SchemeStatus::BufferTooSmall
    );
    let mut buf = vec![0u8; need];
    let mut cap = need;
    assert_eq!(
        unsafe {
            scheme_list(
                h,
                prefix.as_ptr() as *const c_char,
                prefix.len(),
                buf.as_mut_ptr(),
                &mut cap,
            )
        },
        SchemeStatus::Ok
    );
    buf.truncate(cap);
    let liste = String::from_utf8(buf).unwrap();
    let zeilen: Vec<&str> = liste.split('\n').collect();
    assert_eq!(zeilen, vec!["raum/a.txt", "raum/tief/b.txt"]);
    unsafe { scheme_close(h) };
}

#[test]
fn describe_returns_leitfaden() {
    let mut need: usize = 0;
    assert_eq!(
        unsafe { scheme_describe(ptr::null_mut(), &mut need) },
        SchemeStatus::BufferTooSmall
    );
    assert!(need > 0);
    let mut buf = vec![0u8; need];
    let mut cap = need;
    assert_eq!(
        unsafe { scheme_describe(buf.as_mut_ptr(), &mut cap) },
        SchemeStatus::Ok
    );
    buf.truncate(cap);
    let text = String::from_utf8(buf).unwrap();
    assert!(text.contains("klar strukturiert"));
    assert!(text.contains("Beschreibung"));
}

#[test]
fn invalid_handle_and_null_args() {
    // Null-Handle => InvalidHandle.
    let p = "a.txt";
    assert_eq!(
        unsafe { scheme_get(ptr::null(), p.as_ptr() as *const c_char, p.len(), ptr::null_mut(), &mut 0usize) },
        SchemeStatus::InvalidHandle
    );
    // Null out_handle bei open => NullArgument.
    assert_eq!(
        unsafe { scheme_open(ptr::null(), 0, ptr::null_mut()) },
        SchemeStatus::NullArgument
    );
    // Close(null) => Ok.
    assert_eq!(unsafe { scheme_close(ptr::null_mut()) }, SchemeStatus::Ok);
}

#[test]
fn invalid_path_is_reported() {
    let (_td, h) = open();
    let bad = "../ausbruch";
    assert_eq!(
        put_struct(h, bad, "x", b"y"),
        SchemeStatus::InvalidPath
    );
    unsafe { scheme_close(h) };
}

#[test]
fn panic_is_caught_at_the_boundary() {
    // Beweist: ein Panic im FFI-Rumpf wird von catch_unwind gefangen (SCHEME_PANIC),
    // statt als UB über die C-Grenze zu unwinden.
    assert_eq!(scheme_force_panic_for_testing(), SchemeStatus::Panic);
}
