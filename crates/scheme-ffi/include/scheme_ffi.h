/*
 * scheme_ffi.h — hand-geschriebener C-Header für scheme-ffi.
 *
 * Maßgeblich: das Gesetzbuch `semantics/scheme.md`. Diese C-ABI ist für das
 * IN-PROZESS-Embedding in EINER Vertrauenszone gedacht: der Einbetter ist die
 * vertrauenswürdige Schicht-darüber (der traversierende/schreibende Agent).
 * Mandantenfähig/reguliert => der Daemon (schemed), nicht diese FFI.
 *
 * scheme ist der VERÄNDERBARE, klar strukturierte Gegenpol zu lakearch: die
 * Grenze exponiert pfadbasierte Primitive (put/get/update/move/delete/list) statt
 * append-only Inhaltsadressierung. Jede Ablage trägt eine PFLICHT-Beschreibung
 * (§4); der Leitfaden (§9) wird über scheme_describe() herausgereicht.
 *
 * ABI-Sicherheit: JEDE Funktion fängt einen Rust-Panic intern ab (catch_unwind)
 * und gibt SCHEME_PANIC zurück — ein Panic propagiert NIE über die C-Grenze.
 *
 * Puffer-Protokoll (scheme_get/scheme_put/scheme_list/scheme_describe): *out_len
 * traegt beim Aufruf die Kapazitaet von out_buf; nach Erfolg die geschriebene
 * Laenge. Ist der Puffer zu klein => SCHEME_BUFFER_TOO_SMALL und *out_len = die
 * benoetigte Laenge (erneut mit groesserem Puffer aufrufen). out_buf darf NULL
 * sein, solange die Eingangs-Kapazitaet 0 ist (reine Laengen-Abfrage).
 *
 * Pfade/Beschreibungen sind UTF-8-Bytes OHNE terminierende NUL, jeweils mit
 * expliziter Laenge uebergeben.
 */
#ifndef SCHEME_FFI_H
#define SCHEME_FFI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Fehlercodes (int32, C-ABI-stabil). SCHEME_OK = 0; alles andere ist ein
 * definierter, mechanischer Fehlerzustand. Werte sind eingefroren (append-only
 * erweitern, nie umnummerieren).
 */
typedef enum SchemeStatus {
    SCHEME_OK = 0,
    SCHEME_PANIC = 1,                 /* gefangener Rust-Panic (statt UB)         */
    SCHEME_NULL_ARGUMENT = 2,         /* Null-Pointer/Argument-Verletzung         */
    SCHEME_INVALID_HANDLE = 3,        /* ungueltiges/geschlossenes Handle         */
    SCHEME_NOT_FOUND = 4,             /* Knoten nicht vorhanden (Abwesenheit)     */
    SCHEME_BUFFER_TOO_SMALL = 5,      /* Puffer zu klein; out_len = benoetigt     */
    SCHEME_DESCRIPTION_REQUIRED = 6,  /* Pflicht-Beschreibung fehlt/leer (§4)     */
    SCHEME_INVALID_PATH = 7,          /* Pfad verletzt die Pfad-Regel (§3)        */
    SCHEME_KIND_CONFLICT = 8,         /* Datei <-> Ordner an demselben Pfad (§2)  */
    SCHEME_ALREADY_EXISTS = 9,        /* Ziel-Pfad bereits belegt (verschieben)   */
    SCHEME_IO = 14,                   /* I/O-Fehler des Dateisystembaums (§5)     */
    SCHEME_MANIFEST_CORRUPT = 15,     /* .scheme.json beschaedigt (§5/§8); HALT   */
    SCHEME_INDEX_ERROR = 16,          /* Fehler des abgeleiteten Index (§8)       */
    SCHEME_OTHER = 99
} SchemeStatus;

/* Opakes Handle auf einen geoeffneten Bestand (§2.4). */
typedef struct SchemeHandle SchemeHandle;

/*
 * Oeffnet (legt bei Bedarf an) einen Bestand auf dir_utf8 (dir_len Bytes UTF-8,
 * OHNE NUL) und schreibt das Handle nach *out_handle. Der Aufrufer MUSS das
 * Handle mit scheme_close() freigeben (RAII). Bei einem Fehler bleibt
 * *out_handle NULL.
 */
SchemeStatus scheme_open(const char *dir_utf8, size_t dir_len,
                         SchemeHandle **out_handle);

/*
 * Schliesst ein Handle und gibt alle Ressourcen frei. NULL => no-op (Ok). Ein
 * doppeltes close desselben Handles ist VERBOTEN.
 */
SchemeStatus scheme_close(SchemeHandle *handle);

/*
 * Legt data (data_len Bytes) als Datei an path (path_len Bytes UTF-8) ab, mit der
 * PFLICHT-Beschreibung desc (desc_len Bytes UTF-8). Leere/nur-Leerraum-desc =>
 * SCHEME_DESCRIPTION_REQUIRED (§4). Fehlende Elternordner werden angelegt.
 */
SchemeStatus scheme_put_structured(const SchemeHandle *handle,
                                   const char *path, size_t path_len,
                                   const char *desc, size_t desc_len,
                                   const uint8_t *data, size_t data_len);

/*
 * Basis-Ablage OHNE Beschreibung (Drop-in, §6.4). path_len == 0 => deterministisch
 * unter eingang/ ablegen; sonst an path ueberschreiben. Der resultierende Pfad
 * wird nach out_path/*out_path_len geschrieben (Puffer-Protokoll).
 */
SchemeStatus scheme_put(const SchemeHandle *handle,
                        const char *path, size_t path_len,
                        const uint8_t *data, size_t data_len,
                        uint8_t *out_path, size_t *out_path_len);

/* Ueberschreibt den Inhalt einer VORHANDENEN Datei; fehlt sie => SCHEME_NOT_FOUND. */
SchemeStatus scheme_update(const SchemeHandle *handle,
                           const char *path, size_t path_len,
                           const uint8_t *data, size_t data_len);

/*
 * Re-keyt einen Knoten von from nach to. to muss frei sein (sonst
 * SCHEME_ALREADY_EXISTS); fehlt from => SCHEME_NOT_FOUND.
 */
SchemeStatus scheme_move(const SchemeHandle *handle,
                         const char *from, size_t from_len,
                         const char *to, size_t to_len);

/*
 * Loescht einen Knoten (rekursiv bei Ordnern). Idempotent: *out_removed (optional,
 * darf NULL sein) erhaelt 1, wenn etwas entfernt wurde, sonst 0. Abwesenheit ist
 * KEIN Fehler.
 */
SchemeStatus scheme_delete(const SchemeHandle *handle,
                           const char *path, size_t path_len,
                           int32_t *out_removed);

/*
 * Setzt/ersetzt die Beschreibung eines vorhandenen Knotens (§4) und loescht die
 * unbeschrieben-Markierung. Leere desc => SCHEME_DESCRIPTION_REQUIRED; fehlt der
 * Knoten => SCHEME_NOT_FOUND.
 */
SchemeStatus scheme_set_description(const SchemeHandle *handle,
                                    const char *path, size_t path_len,
                                    const char *desc, size_t desc_len);

/*
 * Liest den Inhalt der Datei an path nach out_buf/*out_len (Puffer-Protokoll).
 * Fehlender Knoten oder ein Ordner => SCHEME_NOT_FOUND (Abwesenheit ist normal).
 */
SchemeStatus scheme_get(const SchemeHandle *handle,
                        const char *path, size_t path_len,
                        uint8_t *out_buf, size_t *out_len);

/*
 * Listet die Datei-Pfade unterhalb prefix (prefix_len == 0 => ganzer Bestand) nach
 * out_buf/*out_len (Puffer-Protokoll), je Pfad eine Zeile, mit '\n' getrennt.
 */
SchemeStatus scheme_list(const SchemeHandle *handle,
                         const char *prefix, size_t prefix_len,
                         uint8_t *out_buf, size_t *out_len);

/*
 * Reicht den Struktur-Leitfaden (§9) nach out_buf/*out_len (Puffer-Protokoll):
 * den Text, mit dem scheme dem Agenten mitteilt, dass die Daten klar strukturiert
 * vorliegen und beschrieben werden muessen. Kein Handle noetig (statisch).
 */
SchemeStatus scheme_describe(uint8_t *out_buf, size_t *out_len);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* SCHEME_FFI_H */
