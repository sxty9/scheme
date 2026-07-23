//! Das **opake Bestands-Handle** der C-ABI.
//!
//! Maßgeblich: das Gesetzbuch `semantics/scheme.md` (§5/§6 der Bestand hinter
//! einem Lese-Schreib-Schloss). Der Aufrufer sieht **nur** einen opaken
//! `*mut SchemeHandle`-Pointer. Konstruktion/Freigabe laufen ausschließlich über
//! [`SchemeHandle::into_raw`]/[`SchemeHandle::drop_raw`] (RAII: genau ein `close`
//! gibt genau einen `open` frei). Diese Schicht ist
//! `#![deny(unsafe_op_in_unsafe_fn)]`.

#![deny(unsafe_op_in_unsafe_fn)]

use scheme_core::Bestand;

/// Das **opake Handle** auf einen geöffneten [`Bestand`]. Über die C-ABI erscheint
/// nur ein `*mut SchemeHandle`; das Feld ist privat.
///
/// Ein Handle besitzt **einen** Bestand (§2.4) und erlaubt gleichzeitige Leser und
/// einen serialisierten Schreiber (§6.1). Lebenszeit ist RAII über
/// [`scheme_open`](crate::scheme_open)/[`scheme_close`](crate::scheme_close).
pub struct SchemeHandle {
    bestand: Bestand,
}

impl SchemeHandle {
    /// Verpackt einen geöffneten Bestand in ein Heap-Handle und gibt den **rohen**
    /// Pointer heraus (Eigentum geht an den C-Aufrufer; Freigabe nur via
    /// [`SchemeHandle::drop_raw`]).
    pub(crate) fn into_raw(bestand: Bestand) -> *mut SchemeHandle {
        Box::into_raw(Box::new(SchemeHandle { bestand }))
    }

    /// Leiht den Bestand hinter einem rohen Handle-Pointer. `None`, wenn `handle`
    /// `null` ist.
    ///
    /// # Safety
    /// `handle` muss `null` **oder** ein lebendes, von [`SchemeHandle::into_raw`]
    /// erzeugtes und noch nicht freigegebenes Handle sein. Die geliehene Referenz
    /// darf das Handle nicht überleben.
    pub(crate) unsafe fn bestand_ref<'a>(handle: *const SchemeHandle) -> Option<&'a Bestand> {
        if handle.is_null() {
            return None;
        }
        // SAFETY: laut Vertrag ein lebendes, ausgerichtetes Handle; die Lebenszeit
        // `'a` ist an den FFI-Aufruf gebunden.
        let h = unsafe { &*handle };
        Some(&h.bestand)
    }

    /// Gibt ein rohes Handle frei (rekonstruiert den `Box` und droppt ihn).
    ///
    /// # Safety
    /// `handle` muss ein lebendes, von [`SchemeHandle::into_raw`] erzeugtes und
    /// **noch nicht** freigegebenes Handle sein (nie `null`, nie doppelt).
    pub(crate) unsafe fn drop_raw(handle: *mut SchemeHandle) {
        // SAFETY: laut Vertrag genau einmaliges Reclaim eines per `Box::into_raw`
        // erzeugten Pointers.
        let boxed = unsafe { Box::from_raw(handle) };
        drop(boxed);
    }
}
