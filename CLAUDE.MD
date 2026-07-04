In dem Ordner `/semantics` wird die Bedeutung von scheme hinterlegt. Dabei handelt es sich um die Architektur des Datenmodells und nicht um ihre technologische Umsetzung.

Maßgeblich und vollständig ist das eine Regelwerk `semantics/scheme.md`. Es ist axiomatisch gefasst und domänen-frei: scheme ist ein **veränderbarer, klar strukturierter** Datenpool (Dateisystembaum-als-Wahrheit, Pfad = Identität, **Pflicht-Beschreibung**), der auf derselben Ebene wie lakearch agiert und mit ihr austauschbar ist — der Gegenpol zu lakearchs append-only, inhaltsadressiertem Substrat. Wie lakearch **wertet scheme nicht**.

Konventionen dieser Repo (wie in der lakearch-Familie):
- **Docs, Kommentare und Commit-Meldungen auf Deutsch; Code-Bezeichner englisch/deutsch gemischt** wie es der Klarheit dient. Die C-ABI trägt englische Symbolnamen (`scheme_*`).
- **Ein grüner Commit pro Phase**: `cargo build` + `cargo test` + `cargo clippy --all-targets -- -D warnings` müssen grün sein, bevor eine Phase als fertig gilt.
- Änderungen an `semantics/scheme.md` sind Änderungen am Gesetz — die Implementierung ist konsistent zu halten; §-Verweise in den Doc-Kommentaren müssen stimmen.
- Architektur-Entscheidungen liegen als ADR unter `docs/adr/` (reversibilitäts-getaggt).
