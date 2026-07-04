//! Der **Bestand** des Daemons: **ein** geteilter [`scheme_core::Bestand`] hinter
//! `Arc`, den jede gRPC-Anfrage billig teilt.
//!
//! Maßgeblich: `semantics/scheme.md` §5/§6. Der Core-Bestand serialisiert
//! Schreibvorgänge bereits intern (§6.1); der Daemon muss daher **keine** eigene
//! Schreib-Pipeline bauen — er reicht jede (blockierende) fs-Operation über
//! [`tokio::task::spawn_blocking`] an den geteilten Bestand.

use std::path::PathBuf;
use std::sync::Arc;

use scheme_core::{Bestand as CoreBestand, SchemeError};
use tonic::Status;

/// Der gemeinsam besessene Daemon-Zustand. Klonbar (`Arc`); der eigentliche
/// Bestand liegt **einmal** dahinter.
#[derive(Clone)]
pub struct Bestand {
    inner: Arc<CoreBestand>,
}

impl Bestand {
    /// Öffnet (legt bei Bedarf an) den einen Bestand des Daemons auf `dir`.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Bestand, SchemeError> {
        Ok(Bestand {
            inner: Arc::new(CoreBestand::oeffnen(dir)?),
        })
    }

    /// Führt eine blockierende Bestands-Operation auf einem Blocking-Thread aus und
    /// bildet einen [`SchemeError`] auf einen gRPC-[`Status`] ab. Der geteilte
    /// `Arc<CoreBestand>` wird in die Closure gezogen; die interne Schreib-Sperre
    /// des Core linearisiert nebenläufige Schreiber (§6.1).
    pub(crate) async fn run<T, F>(&self, f: F) -> Result<T, Status>
    where
        F: FnOnce(&CoreBestand) -> Result<T, SchemeError> + Send + 'static,
        T: Send + 'static,
    {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || f(inner.as_ref()))
            .await
            .map_err(|e| Status::internal(format!("spawn_blocking join: {e}")))?
            .map_err(status_from)
    }

    /// Der Struktur-Leitfaden (§9) — statisch, ohne Blocking.
    pub(crate) fn leitfaden(&self) -> &'static str {
        self.inner.leitfaden()
    }
}

/// Bildet einen mechanischen [`SchemeError`] auf einen gRPC-[`Status`] ab. scheme
/// wertet nicht (§1.4) — die Codes sind kategorisch, nicht inhaltlich.
pub(crate) fn status_from(e: SchemeError) -> Status {
    match e {
        SchemeError::BeschreibungFehlt => Status::invalid_argument(e.to_string()),
        SchemeError::UngueltigerPfad(_) => Status::invalid_argument(e.to_string()),
        SchemeError::NichtGefunden(_) => Status::not_found(e.to_string()),
        SchemeError::ArtKonflikt { .. } => Status::failed_precondition(e.to_string()),
        SchemeError::BereitsVorhanden(_) => Status::already_exists(e.to_string()),
        SchemeError::ManifestBeschaedigt(_) => Status::data_loss(e.to_string()),
        SchemeError::Io(_) => Status::internal(e.to_string()),
        SchemeError::IndexFehler(_) => Status::internal(e.to_string()),
        // `SchemeError` ist `#[non_exhaustive]`: unbekannter Zustand ⇒ internal.
        _ => Status::internal(e.to_string()),
    }
}
