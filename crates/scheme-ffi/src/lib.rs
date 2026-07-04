//! scheme-ffi — die **C-ABI** für das **IN-PROZESS-Embedding** von scheme.
//!
//! Maßgeblich: das Gesetzbuch `semantics/scheme.md`. Die Grenze exponiert die
//! **veränderbaren, pfadbasierten** scheme-Primitive über **opake Handle-Pointer**
//! — der Gegenpol zu lakearchs append-only, inhaltsadressierter C-ABI:
//! - [`scheme_open`]/[`scheme_close`] — einen Bestand öffnen/schließen (RAII).
//! - [`scheme_put_structured`] — eine Datei mit **Pflicht-Beschreibung** ablegen (§4/§6).
//! - [`scheme_put`] — Basis-Ablage ohne Beschreibung (Drop-in, §6.4); liefert den
//!   zugewiesenen Pfad (Puffer-Protokoll).
//! - [`scheme_get`] — den Inhalt einer Datei lesen (Puffer-Protokoll; VANISH:
//!   Abwesenheit ⇒ [`SchemeStatus::NotFound`]).
//! - [`scheme_update`]/[`scheme_move`]/[`scheme_delete`] — verändern (§6).
//! - [`scheme_set_description`] — die Beschreibung eines Knotens setzen (§4).
//! - [`scheme_list`] — Datei-Pfade unter einem Präfix auflisten (§7; `\n`-getrennt).
//! - [`scheme_describe`] — den Struktur-**Leitfaden** (§9) herausreichen.
//!
//! Diese C-ABI ist für ein In-Prozess-Embedding in **einer** Vertrauenszone
//! gedacht (der Einbetter ist die vertrauenswürdige Schicht darüber). Multi-Tenant /
//! Netz ⇒ der Daemon (`schemed`), nicht diese FFI.
//!
//! ## ABI-Sicherheit: kein Panic über die C-Grenze (UB-Schutz)
//!
//! Ein über die C-ABI **unwindender** Rust-Panic ist **undefiniertes Verhalten**.
//! Daher umschließt **jede** `extern "C"`-Funktion ihren Rumpf in
//! [`std::panic::catch_unwind`] und wandelt einen Panic in [`SchemeStatus::Panic`].
//! In den FFI-Pfaden gibt es **kein** `unwrap`/`expect`/`panic!`; Fehler sind
//! ausschließlich Rückgabe-Codes. Diese Schicht ist
//! `#![deny(unsafe_op_in_unsafe_fn)]`.

#![deny(unsafe_op_in_unsafe_fn)]

mod error;
mod handle;

pub use error::SchemeStatus;
pub use handle::SchemeHandle;

use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::slice;

use scheme_core::{Bestand, Beschreibung, Pfad, LEITFADEN};

/// **Panik-Wächter**: führt `body` unter [`catch_unwind`] aus; ein Panic wird
/// gefangen und als [`SchemeStatus::Panic`] zurückgegeben (statt UB über die
/// C-Grenze).
fn guard<F: FnOnce() -> SchemeStatus>(body: F) -> SchemeStatus {
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(status) => status,
        Err(_) => SchemeStatus::Panic,
    }
}

// ---------------------------------------------------------------------------
// Öffnen / Schließen — RAII über das opake Handle.
// ---------------------------------------------------------------------------

/// Öffnet (legt bei Bedarf an) einen Bestand auf `dir_utf8` (`dir_len` Bytes,
/// UTF-8, **ohne** NUL) und schreibt das opake Handle nach `*out_handle`. Der
/// Aufrufer **muss** es mit [`scheme_close`] freigeben.
///
/// # Safety
/// `dir_utf8` muss auf `dir_len` lesbare Bytes zeigen (oder `dir_len == 0`).
/// `out_handle` muss auf einen beschreibbaren `*mut SchemeHandle` zeigen. Bei
/// einem anderen Code als [`SchemeStatus::Ok`] wird `*out_handle` auf `null` gesetzt.
#[no_mangle]
pub unsafe extern "C" fn scheme_open(
    dir_utf8: *const c_char,
    dir_len: usize,
    out_handle: *mut *mut SchemeHandle,
) -> SchemeStatus {
    guard(|| {
        if out_handle.is_null() {
            return SchemeStatus::NullArgument;
        }
        // SAFETY: laut Vertrag beschreibbar; vorsorglich auf null, damit ein
        // Fehlerpfad nie ein wildes Handle hinterlässt.
        unsafe { *out_handle = std::ptr::null_mut() };

        let dir = match unsafe { read_str(dir_utf8, dir_len) } {
            Ok(s) => s,
            Err(st) => return st,
        };
        match Bestand::oeffnen(dir) {
            Ok(b) => {
                let handle = SchemeHandle::into_raw(b);
                // SAFETY: `out_handle` ist (oben geprüft) nicht-null und beschreibbar.
                unsafe { *out_handle = handle };
                SchemeStatus::Ok
            }
            Err(e) => SchemeStatus::from(e),
        }
    })
}

/// Schließt ein mit [`scheme_open`] geöffnetes Handle und gibt alle Ressourcen
/// frei. Ein doppelter `close` ist **verboten**; `null` ⇒ no-op ([`SchemeStatus::Ok`]).
///
/// # Safety
/// `handle` muss `null` **oder** ein von [`scheme_open`] geliefertes, noch nicht
/// geschlossenes Handle sein.
#[no_mangle]
pub unsafe extern "C" fn scheme_close(handle: *mut SchemeHandle) -> SchemeStatus {
    guard(|| {
        if handle.is_null() {
            return SchemeStatus::Ok;
        }
        // SAFETY: laut Vertrag ein gültiges, noch nicht freigegebenes Handle.
        unsafe { SchemeHandle::drop_raw(handle) };
        SchemeStatus::Ok
    })
}

// ---------------------------------------------------------------------------
// Schreiben (§6).
// ---------------------------------------------------------------------------

/// **Strukturiert ablegen** (§6): schreibt `data` als Datei an `path` mit der
/// **Pflicht-Beschreibung** `desc`. Eine leere/nur-Leerraum-Beschreibung ⇒
/// [`SchemeStatus::DescriptionRequired`] (§4). Fehlende Elternordner werden angelegt.
///
/// # Safety
/// `path`/`desc`/`data` müssen auf ihre jeweilige Länge lesbare Bytes zeigen (oder
/// die Länge ist 0).
#[no_mangle]
pub unsafe extern "C" fn scheme_put_structured(
    handle: *const SchemeHandle,
    path: *const c_char,
    path_len: usize,
    desc: *const c_char,
    desc_len: usize,
    data: *const u8,
    data_len: usize,
) -> SchemeStatus {
    guard(|| {
        let bestand = match unsafe { SchemeHandle::bestand_ref(handle) } {
            Some(b) => b,
            None => return SchemeStatus::InvalidHandle,
        };
        let pfad = match unsafe { read_pfad(path, path_len) } {
            Ok(p) => p,
            Err(st) => return st,
        };
        let desc = match unsafe { read_str(desc, desc_len) } {
            Ok(s) => s,
            Err(st) => return st,
        };
        let bytes = match unsafe { read_bytes(data, data_len) } {
            Ok(d) => d,
            Err(st) => return st,
        };
        let beschreibung = match Beschreibung::neu(desc) {
            Ok(b) => b,
            Err(e) => return SchemeStatus::from(e),
        };
        match bestand.ablegen(&pfad, beschreibung, bytes) {
            Ok(_) => SchemeStatus::Ok,
            Err(e) => SchemeStatus::from(e),
        }
    })
}

/// **Basis-Ablage** (Drop-in, §6.4): schreibt `data` ohne Beschreibung. Ist
/// `path_len == 0`, wird deterministisch unter `eingang/` abgelegt; sonst an
/// `path` überschrieben. Der resultierende Pfad wird nach `out_path`/`*out_path_len`
/// geschrieben (Puffer-Protokoll: `*out_path_len` trägt beim Aufruf die Kapazität,
/// nach Erfolg die Länge; zu klein ⇒ [`SchemeStatus::BufferTooSmall`] + benötigte Länge).
///
/// # Safety
/// `path`/`data` gültig für ihre Längen. `out_path_len` muss auf einen les-/
/// beschreibbaren `usize` zeigen; `out_path` auf `*out_path_len` beschreibbare Bytes
/// (oder `null`, wenn der Eingangswert 0 ist).
#[no_mangle]
pub unsafe extern "C" fn scheme_put(
    handle: *const SchemeHandle,
    path: *const c_char,
    path_len: usize,
    data: *const u8,
    data_len: usize,
    out_path: *mut u8,
    out_path_len: *mut usize,
) -> SchemeStatus {
    guard(|| {
        let bestand = match unsafe { SchemeHandle::bestand_ref(handle) } {
            Some(b) => b,
            None => return SchemeStatus::InvalidHandle,
        };
        let bytes = match unsafe { read_bytes(data, data_len) } {
            Ok(d) => d,
            Err(st) => return st,
        };
        let ergebnis = if path_len == 0 {
            bestand.ablegen_roh(None, bytes)
        } else {
            let pfad = match unsafe { read_pfad(path, path_len) } {
                Ok(p) => p,
                Err(st) => return st,
            };
            bestand.ablegen_roh(Some(&pfad), bytes)
        };
        match ergebnis {
            Ok(k) => unsafe { write_out(k.pfad.als_str().as_bytes(), out_path, out_path_len) },
            Err(e) => SchemeStatus::from(e),
        }
    })
}

/// **Aktualisieren** (§6): überschreibt den Inhalt einer vorhandenen Datei; fehlt
/// sie ⇒ [`SchemeStatus::NotFound`].
///
/// # Safety
/// `path`/`data` gültig für ihre Längen.
#[no_mangle]
pub unsafe extern "C" fn scheme_update(
    handle: *const SchemeHandle,
    path: *const c_char,
    path_len: usize,
    data: *const u8,
    data_len: usize,
) -> SchemeStatus {
    guard(|| {
        let bestand = match unsafe { SchemeHandle::bestand_ref(handle) } {
            Some(b) => b,
            None => return SchemeStatus::InvalidHandle,
        };
        let pfad = match unsafe { read_pfad(path, path_len) } {
            Ok(p) => p,
            Err(st) => return st,
        };
        let bytes = match unsafe { read_bytes(data, data_len) } {
            Ok(d) => d,
            Err(st) => return st,
        };
        match bestand.aktualisieren(&pfad, bytes) {
            Ok(_) => SchemeStatus::Ok,
            Err(e) => SchemeStatus::from(e),
        }
    })
}

/// **Verschieben/Umbenennen** (§6): re-keyt einen Knoten von `from` nach `to`.
/// `to` muss frei sein (sonst [`SchemeStatus::AlreadyExists`]); fehlt `from` ⇒
/// [`SchemeStatus::NotFound`].
///
/// # Safety
/// `from`/`to` gültig für ihre Längen.
#[no_mangle]
pub unsafe extern "C" fn scheme_move(
    handle: *const SchemeHandle,
    from: *const c_char,
    from_len: usize,
    to: *const c_char,
    to_len: usize,
) -> SchemeStatus {
    guard(|| {
        let bestand = match unsafe { SchemeHandle::bestand_ref(handle) } {
            Some(b) => b,
            None => return SchemeStatus::InvalidHandle,
        };
        let von = match unsafe { read_pfad(from, from_len) } {
            Ok(p) => p,
            Err(st) => return st,
        };
        let nach = match unsafe { read_pfad(to, to_len) } {
            Ok(p) => p,
            Err(st) => return st,
        };
        match bestand.verschieben(&von, &nach) {
            Ok(()) => SchemeStatus::Ok,
            Err(e) => SchemeStatus::from(e),
        }
    })
}

/// **Löschen** (§6): entfernt einen Knoten (rekursiv bei Ordnern). Idempotent:
/// `*out_removed` (optional, darf `null` sein) erhält `1`, wenn etwas entfernt
/// wurde, sonst `0`. Ein abwesender Knoten ist **kein** Fehler.
///
/// # Safety
/// `path` gültig für seine Länge. `out_removed` muss `null` oder ein beschreibbarer
/// `i32` sein.
#[no_mangle]
pub unsafe extern "C" fn scheme_delete(
    handle: *const SchemeHandle,
    path: *const c_char,
    path_len: usize,
    out_removed: *mut i32,
) -> SchemeStatus {
    guard(|| {
        let bestand = match unsafe { SchemeHandle::bestand_ref(handle) } {
            Some(b) => b,
            None => return SchemeStatus::InvalidHandle,
        };
        let pfad = match unsafe { read_pfad(path, path_len) } {
            Ok(p) => p,
            Err(st) => return st,
        };
        match bestand.loeschen(&pfad) {
            Ok(entfernt) => {
                if !out_removed.is_null() {
                    // SAFETY: oben als nicht-null geprüft, laut Vertrag beschreibbar.
                    unsafe { *out_removed = i32::from(entfernt) };
                }
                SchemeStatus::Ok
            }
            Err(e) => SchemeStatus::from(e),
        }
    })
}

/// **Beschreiben** (§4): setzt/ersetzt die Beschreibung eines vorhandenen Knotens
/// und löscht die `unbeschrieben`-Markierung. Leere Beschreibung ⇒
/// [`SchemeStatus::DescriptionRequired`]; fehlt der Knoten ⇒ [`SchemeStatus::NotFound`].
///
/// # Safety
/// `path`/`desc` gültig für ihre Längen.
#[no_mangle]
pub unsafe extern "C" fn scheme_set_description(
    handle: *const SchemeHandle,
    path: *const c_char,
    path_len: usize,
    desc: *const c_char,
    desc_len: usize,
) -> SchemeStatus {
    guard(|| {
        let bestand = match unsafe { SchemeHandle::bestand_ref(handle) } {
            Some(b) => b,
            None => return SchemeStatus::InvalidHandle,
        };
        let pfad = match unsafe { read_pfad(path, path_len) } {
            Ok(p) => p,
            Err(st) => return st,
        };
        let desc = match unsafe { read_str(desc, desc_len) } {
            Ok(s) => s,
            Err(st) => return st,
        };
        let beschreibung = match Beschreibung::neu(desc) {
            Ok(b) => b,
            Err(e) => return SchemeStatus::from(e),
        };
        match bestand.beschreiben(&pfad, beschreibung) {
            Ok(_) => SchemeStatus::Ok,
            Err(e) => SchemeStatus::from(e),
        }
    })
}

// ---------------------------------------------------------------------------
// Lesen (§7).
// ---------------------------------------------------------------------------

/// **Lesen** (§7): füllt `out_buf`/`*out_len` mit dem Inhalt der Datei an `path`
/// (Puffer-Protokoll). Ein fehlender Knoten oder ein Ordner ⇒
/// [`SchemeStatus::NotFound`] (Abwesenheit ist ein normales Ergebnis).
///
/// # Safety
/// `path` gültig für seine Länge. `out_len` muss auf einen les-/beschreibbaren
/// `usize` zeigen; `out_buf` auf `*out_len` beschreibbare Bytes (oder `null`, wenn
/// der Eingangswert 0 ist).
#[no_mangle]
pub unsafe extern "C" fn scheme_get(
    handle: *const SchemeHandle,
    path: *const c_char,
    path_len: usize,
    out_buf: *mut u8,
    out_len: *mut usize,
) -> SchemeStatus {
    guard(|| {
        let bestand = match unsafe { SchemeHandle::bestand_ref(handle) } {
            Some(b) => b,
            None => return SchemeStatus::InvalidHandle,
        };
        let pfad = match unsafe { read_pfad(path, path_len) } {
            Ok(p) => p,
            Err(st) => return st,
        };
        match bestand.lesen(&pfad) {
            Ok(Some(bytes)) => unsafe { write_out(&bytes, out_buf, out_len) },
            Ok(None) => SchemeStatus::NotFound,
            Err(e) => SchemeStatus::from(e),
        }
    })
}

/// **Auflisten** (§7): füllt `out_buf`/`*out_len` mit den Datei-Pfaden unterhalb
/// `prefix`, je Pfad eine Zeile, mit `\n` getrennt (Puffer-Protokoll). Ein leerer
/// Präfix (`prefix_len == 0`) listet den ganzen Bestand.
///
/// # Safety
/// `prefix` gültig für seine Länge. `out_len`/`out_buf` wie bei [`scheme_get`].
#[no_mangle]
pub unsafe extern "C" fn scheme_list(
    handle: *const SchemeHandle,
    prefix: *const c_char,
    prefix_len: usize,
    out_buf: *mut u8,
    out_len: *mut usize,
) -> SchemeStatus {
    guard(|| {
        let bestand = match unsafe { SchemeHandle::bestand_ref(handle) } {
            Some(b) => b,
            None => return SchemeStatus::InvalidHandle,
        };
        let pfad = if prefix_len == 0 {
            Pfad::wurzel()
        } else {
            match unsafe { read_pfad(prefix, prefix_len) } {
                Ok(p) => p,
                Err(st) => return st,
            }
        };
        match bestand.auflisten(&pfad) {
            Ok(pfade) => {
                let joined = pfade
                    .iter()
                    .map(|p| p.als_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                unsafe { write_out(joined.as_bytes(), out_buf, out_len) }
            }
            Err(e) => SchemeStatus::from(e),
        }
    })
}

/// Reicht den Struktur-**Leitfaden** (§9) heraus: den Text, mit dem scheme dem
/// Agenten mitteilt, dass die Daten klar strukturiert vorliegen und beschrieben
/// werden müssen. Kein Handle nötig (statisch). Puffer-Protokoll wie [`scheme_get`].
///
/// # Safety
/// `out_len` muss auf einen les-/beschreibbaren `usize` zeigen; `out_buf` auf
/// `*out_len` beschreibbare Bytes (oder `null`, wenn der Eingangswert 0 ist).
#[no_mangle]
pub unsafe extern "C" fn scheme_describe(out_buf: *mut u8, out_len: *mut usize) -> SchemeStatus {
    guard(|| unsafe { write_out(LEITFADEN.as_bytes(), out_buf, out_len) })
}

// ---------------------------------------------------------------------------
// Test-Stütze: erzwingt einen Panic im FFI-Rumpf, um zu beweisen, dass
// `catch_unwind` ihn fängt (statt UB über die C-Grenze).
// ---------------------------------------------------------------------------

#[doc(hidden)]
#[no_mangle]
pub extern "C" fn scheme_force_panic_for_testing() -> SchemeStatus {
    guard(|| {
        panic!("erzwungener Panic im FFI-Rumpf (nur Test): muss von catch_unwind gefangen werden");
    })
}

// ---------------------------------------------------------------------------
// Interne Helfer.
// ---------------------------------------------------------------------------

/// Liest einen UTF-8-String aus `(ptr, len)`.
///
/// # Safety
/// `ptr` muss auf `len` lesbare Bytes zeigen (oder `len == 0`).
unsafe fn read_str<'a>(ptr: *const c_char, len: usize) -> Result<&'a str, SchemeStatus> {
    if ptr.is_null() && len != 0 {
        return Err(SchemeStatus::NullArgument);
    }
    let bytes: &[u8] = if len == 0 {
        &[]
    } else {
        // SAFETY: laut Vertrag `len` lesbare Bytes ab `ptr`.
        unsafe { slice::from_raw_parts(ptr as *const u8, len) }
    };
    std::str::from_utf8(bytes).map_err(|_| SchemeStatus::InvalidPath)
}

/// Liest und **validiert** einen Pfad (§3) aus `(ptr, len)`.
///
/// # Safety
/// wie [`read_str`].
unsafe fn read_pfad(ptr: *const c_char, len: usize) -> Result<Pfad, SchemeStatus> {
    let s = unsafe { read_str(ptr, len) }?;
    Pfad::parse(s).map_err(SchemeStatus::from)
}

/// Liest einen Byte-Slice aus `(ptr, len)`.
///
/// # Safety
/// `ptr` muss auf `len` lesbare Bytes zeigen (oder `len == 0`).
unsafe fn read_bytes<'a>(ptr: *const u8, len: usize) -> Result<&'a [u8], SchemeStatus> {
    if ptr.is_null() && len != 0 {
        return Err(SchemeStatus::NullArgument);
    }
    Ok(if len == 0 {
        &[]
    } else {
        // SAFETY: laut Vertrag `len` lesbare Bytes ab `ptr`.
        unsafe { slice::from_raw_parts(ptr, len) }
    })
}

/// **Puffer-Protokoll**: schreibt `bytes` nach `out_buf`, wenn die Kapazität
/// (`*out_len` beim Aufruf) reicht; setzt `*out_len` immer auf die benötigte Länge.
/// Zu klein ⇒ [`SchemeStatus::BufferTooSmall`] ohne Datenverlust.
///
/// # Safety
/// `out_len` muss auf einen les-/beschreibbaren `usize` zeigen; `out_buf` auf
/// mindestens `*out_len` (Eingangswert) beschreibbare Bytes (oder `null`, wenn 0).
unsafe fn write_out(bytes: &[u8], out_buf: *mut u8, out_len: *mut usize) -> SchemeStatus {
    if out_len.is_null() {
        return SchemeStatus::NullArgument;
    }
    // SAFETY: oben als nicht-null geprüft, laut Vertrag lesbar.
    let capacity = unsafe { *out_len };
    if out_buf.is_null() && capacity != 0 {
        return SchemeStatus::NullArgument;
    }
    let needed = bytes.len();
    // SAFETY: `out_len` nicht-null (oben) und beschreibbar.
    unsafe { *out_len = needed };
    if needed > capacity {
        return SchemeStatus::BufferTooSmall;
    }
    if needed > 0 {
        // SAFETY: `out_buf` hält laut Vertrag `capacity >= needed` beschreibbare
        // Bytes; Quelle/Ziel überlappen nicht (frische FFI-Puffer).
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, needed) };
    }
    SchemeStatus::Ok
}
