# scheme — Das Datenmodell

> Regelwerk. Vollständig, technologie-arm gefasst. scheme ist der veränderbare, klar strukturierte Gegenpol zu lakearch.

**Präambel.** Dieses Dokument beschreibt das Datenmodell *scheme* erschöpfend. scheme steht **auf derselben Ebene wie lakearch** und ist mit ihr **austauschbar** (§11): beide sind Substrate, die Daten entgegennehmen, speichern und wieder herausgeben, ohne sie zu werten. Der Unterschied ist der Charakter der Ablage. lakearch ist **append-only** und **inhaltsadressiert**: es legt Daten in einer „wilden", per Hash adressierten, nicht menschlich navigierbaren Struktur ab. scheme ist **voll veränderbar** und **pfadadressiert**: es legt Daten in einer **klaren, menschlich navigierbaren Hierarchie** ab (echte Ordner, echte Dateien) und lässt sie lesen, ändern, verschieben und löschen. Wie lakearch **wertet scheme nicht**.

**Notation.** Ein *Knoten* ist die einzige Entität. „`a/b/c`" ist ein *Pfad*. Beispiele sind rein illustrativ; scheme kennt keine Domäne.

---

## §1 Geltungsbereich

1.1 scheme **speichert** Daten in einer klar strukturierten, benannten Hierarchie.
1.2 scheme ist **voll lesbar und schreibbar**: die Ablage ist veränderbar. Ablegen, Lesen, Aktualisieren, Verschieben, Löschen und Auflisten sind erststellige Vorgänge (§6/§7). Dies ist der Gegensatz zu lakearchs strikter Append-only-Regel.
1.3 scheme **traversiert** die Hierarchie deterministisch (§7). Der Baum ist azyklisch (Eltern → Kind), Traversierung terminiert strukturell.
1.4 scheme **rechnet nicht** (keine Arithmetik, Aggregation), **wertet nicht** (erzeugt keine Konfidenz, entscheidet keine Relevanz, deutet keinen Inhalt) und **sortiert nicht** nach Wert. Es ordnet Ausgaben allein deterministisch nach Pfad.
1.5 Alles Rechnen, Werten und Entscheiden — auch **wohin** ein Datum gehört und **wie** es zu beschreiben ist — liegt in einer **Schicht über scheme**. Diese Schicht ist der traversierende und schreibende **Agent** (eine aigentic-Instanz). scheme *nimmt* die fertige Struktur und Beschreibung *entgegen*; es *erfindet* sie nicht.
1.6 scheme hat **eine aktive Besonderheit** gegenüber einem stummen Speicher: zur Traversier- und Schreibzeit **teilt es dem Agenten mit**, dass die Daten in klarer strukturierter Form vorliegen und auch so beschrieben werden **müssen** (§9), und es **erzwingt** die Beschreibung (§4).

## §2 Die Entität

2.1 Es gibt genau **eine** Art von Entität: den **Knoten**. Alles, was existiert, ist ein Knoten.
2.2 Ein Knoten hat genau eine von zwei **Arten**: **Ordner** (ein benannter Elternknoten, der Kinder hält) oder **Datei** (ein abgelegtes Datum, das an einem Blattpfad Bytes trägt).
2.3 Eine Art wird **nie still in die andere umgewandelt**. Wo an einem Pfad die andere Art läge, ist das ein Art-Konflikt (ein mechanischer Fehler), keine implizite Umdeutung.
2.4 Ein **Bestand** ist ein zusammenhängender Knotenbaum unter genau einer Wurzel — eine scheme-Instanz.

## §3 Der Pfad

3.1 Jeder Knoten ist über seinen **Pfad** identifiziert: eine normalisierte, relative, mit `/` getrennte Folge von Segmenten. Der leere Pfad ist die **Wurzel**.
3.2 Der Pfad ist **menschenlesbar** und **hierarchisch** (`praesentationsraum/hardware/computer`). Er ist zugleich die Identität *und* die navigierbare Adresse — es gibt keine zweite, opake Adressebene.
3.3 Ein Pfad enthält **keine** leeren Segmente, **kein** `.` oder `..`, **keinen** führenden/abschließenden `/` und keine reservierten internen Namen. Daraus folgt **Confinement**: ein Pfad zeigt nie aus dem Bestand heraus.
3.4 Der Pfad ist **stabil über Inhaltsänderungen** (§6). Ein Verschieben (§6) ist ein *Re-Key*: die Identität wandert mit dem Pfad. Anders als bei lakearch ist die Identität **vom Inhalt entkoppelt** — dasselbe Datum kann in-place geändert werden, ohne die Identität zu wechseln, und zwei inhaltsgleiche Dateien an zwei Pfaden sind zwei **unabhängige** Knoten (keine automatische Deduplizierung).

## §4 Die Beschreibung

4.1 Jeder Knoten trägt eine **Beschreibung**: einen klaren, menschenlesbaren Text, der ihn strukturell einordnet — was er ist und wohin er gehört.
4.2 Die Beschreibung ist beim Ablegen **Pflicht** und **maschinell erzwungen**: ein Schreibvorgang mit leerer oder nur aus Leerraum bestehender Beschreibung wird **abgelehnt**. Dies ist die konkrete Umsetzung des Satzes „Daten liegen in klarer strukturierter Form vor und müssen auch so beschrieben werden".
4.3 scheme **beurteilt die Beschreibung nicht** (§1.4) — es prüft allein, dass sie **vorhanden** und nicht leer ist. Ihre inhaltliche Güte liegt beim Agenten (§1.5).
4.4 Ein Knoten, der über den Basis-Vertrag (§6.4) ohne Beschreibung eintrifft, erhält eine automatisch abgeleitete **Platzhalter-Beschreibung** und wird als *unbeschrieben* markiert — ein sichtbares Signal, dass er noch klar einzuordnen ist. Das Nachreichen einer echten Beschreibung löscht die Markierung.

## §5 Die Wahrheit

5.1 Die **Wahrheit** eines Bestands ist ein **echter Verzeichnisbaum** auf der Platte: Ordner sind echte Verzeichnisse, Dateien echte Dateien. Ein Mensch kann den Bestand mit den Mitteln des Dateisystems (`ls`, `cd`, Öffnen einer Datei) navigieren und lesen — scheme ist **komplett lesbar**.
5.2 Neben jedem Verzeichnis liegt ein **Manifest** (`.scheme.json`), das jedes Kind auf seine Metadaten abbildet: Art, **Beschreibung**, Größe, Zeitstempel, optionalen MIME-Typ und Herkunft. Das Manifest ist selbst menschenlesbar (sortierte, deutsche Schlüssel).
5.3 Dies ist die bewusste **Inversion** von lakearchs „Log-als-Wahrheit": scheme hält dieselbe Disziplin von *einer* autoritativen Wahrheit, aber die Wahrheit ist der Baum, nicht ein append-only Log.

## §6 Das Schreiben

6.1 Alle Zugriffe laufen durch **ein Lese-Schreib-Schloss**: Schreibvorgänge sind **serialisiert** (exklusiv, genau einer zur Zeit), Lesevorgänge laufen **nebenläufig untereinander, aber nie gleichzeitig mit einem Schreibvorgang**. Dadurch ist **jeder** lesende und schreibende Zugriff **atomar und unteilbar** — ein Leser beobachtet **nie** einen Zwischenzustand einer mehrschrittigen Mutation (etwa das `rename` eines Verschiebens vor der Aktualisierung der beteiligten Manifeste, oder die Datei vor ihrem Manifest-Eintrag). Zusätzlich ist jeder einzelne Datei- und Manifest-Schreibvorgang für sich atomar (Temp-Datei + `rename`), so dass selbst ein nicht koordinierter **anderer Prozess** (§12) nie eine halb geschriebene Datei sieht. Die Koordination gilt **prozess-intern** (§12).
6.2 Die Schreib-Primitive sind: **ablegen** (eine Datei mit Pflicht-Beschreibung schreiben, Elternordner bei Bedarf anlegen), **aktualisieren** (den Inhalt einer vorhandenen Datei überschreiben, Beschreibung bleibt), **verschieben** (einen Knoten re-keyen), **löschen** (einen Knoten, rekursiv bei Ordnern; idempotent) und **beschreiben** (die Beschreibung eines Knotens setzen/ersetzen).
6.3 Löschen ist **hart** (der Vorgabe nach): der Knoten wird physisch entfernt. scheme „wertet nicht" und hält daher keine Versionshistorie im Modell vor; Wiederherstellbarkeit ist eine optionale Betriebsentscheidung (§12), kein Modell-Bestandteil.
6.4 **Basis-Vertrag (Drop-in).** Damit scheme am Minimal-Seam (§11) mit lakearch austauschbar bleibt, gibt es ein **rohes Ablegen** ohne Beschreibung: ohne Pfad wird deterministisch unter dem wohlbekannten `eingang/`-Ordner abgelegt (Name aus dem Inhalt abgeleitet), mit Platzhalter-Beschreibung und *unbeschrieben*-Markierung (§4.4); mit Pfad wird überschrieben, eine bereits vorhandene Beschreibung bleibt erhalten. Der **strukturierte** Pfad (mit Pflicht-Beschreibung) ist der eigentlich intendierte; der Basis-Vertrag ist der Fallback.

## §7 Das Lesen & Traversieren

7.1 **Lesen** liefert den Inhalt der Datei an einem Pfad. **Abwesenheit ist ein normales Ergebnis** (kein Knoten / ein Ordner ⇒ „nichts"), kein Fehler.
7.2 **Auflisten** liefert alle Datei-Pfade unterhalb eines Präfixes; **Kinder** liefert die direkten Kinder eines Ordners; **Traversieren** läuft den Teilbaum ab. Alle drei liefern **deterministisch nach Pfad geordnet** (§1.4).
7.3 Traversieren ist **beschränkt** (Tiefe- und Knoten-Budget): definierter Abbruch statt unbeschränkter Lauf.

## §8 Der Index

8.1 Ein Bestand darf einen **Index** halten (Pfad → Metadaten, Volltext über Beschreibungen) zur schnellen Suche.
8.2 Der Index ist ein **reines, neu-baubares Derivat**: er wird durch Ablaufen des Baums (§5) erzeugt und kann jederzeit verworfen und neu gebaut werden. Der Baum wird **nie** aus dem Index rekonstruiert — der Index ist eine Projektion des Baums.

## §9 Der Leitfaden

9.1 scheme hält einen festen **Leitfaden**: den Text, mit dem es dem Agenten mitteilt, dass die Daten klar strukturiert vorliegen und beim Ablegen ein klarer Pfad **und** eine klare Beschreibung anzugeben sind.
9.2 Der Leitfaden wird über die Grenzen (C-ABI, Daemon) nach außen gereicht, damit die Schicht darüber (die aigentic-Kontext-Zusammenstellung) ihn in den Agenten-Kontext spritzt — zusätzlich zur maschinellen Erzwingung der Beschreibung (§4). Der Leitfaden **lehrt**, die Erzwingung **garantiert**.

## §10 Ableitung & Trennung

10.1 Eine Domäne (etwa „präsentationsraum/hardware/…") erweitert das Modell nicht. Sie ist allein **Struktur** — Pfade und Beschreibungen — *innerhalb* eines scheme-Bestands.
10.2 **Trennungsregel.** Ein neues Speicher-, Lese- oder Traversier-Primitiv gehört in scheme und muss domänen-frei sein. Das Wählen von Pfad und Beschreibung, jedes Werten und Entscheiden gehört in die Schicht darüber (§1.5).

## §11 Austauschbarkeit

11.1 scheme ist mit lakearch **austauschbar**: beide erfüllen denselben Substrat-Vertrag der Schicht darüber (das `graveyard.Graveyard`-Interface der aigentic-/prizm-Ebene) — ein Datum unter einer Referenz speichern und wieder herauslesen.
11.2 Wo lakearch die Referenz als Inhalts-Hash vergibt und append-only ist, ehrt scheme die Referenz als **Pfad** und ist **veränderbar**: es erfüllt zusätzlich die optionalen Fähigkeiten **löschbar** und **auflistbar**, die ein append-only Substrat auslässt.
11.3 Scheme-eigene, reichere Vorgänge (Pfad + Pflicht-Beschreibung, Verschieben, der Leitfaden) liegen in schemes **eigenem** Interface — nicht im gemeinsamen Minimal-Vertrag.

## §12 Bewusst offen (Implementierung & Betrieb)

Kein Modell-Bestandteil, sondern Umsetzungs-Entscheidungen:
- **Wiederherstellbarkeit** gelöschter/überschriebener Knoten (Papierkorb, Audit-Spur) — die eine Stelle, an der scheme optional lakearchs append-only Ehrlichkeit borgen könnte.
- **Index-Technik & Leistung** (Volltext, Caching).
- **Nebenläufigkeit** über das prozess-interne Lese-Schreib-Schloss (§6.1) hinaus: mehrere **Prozesse** auf derselben Wurzel sind nicht koordiniert.
- **Absturz-Konsistenz mehrschrittiger Mutationen.** Einzelne Datei- und Manifest-Schreibvorgänge sind atomar (§6.1); ein Vorgang, der *mehrere* schreibt (Verschieben über Verzeichnisgrenzen: `rename` + zwei Manifeste), ist in v1 **nicht** transaktional. Ein Absturz im Fenster hinterlässt eine reparierbare Inkonsistenz (verwaiste Datei oder Eintrag), nie Datenverlust; die Reihenfolge ist bewusst so gewählt, dass der Baum die Wahrheit bleibt und ein Index-Neubau (§8) wieder aufräumt. Ein Journal/WAL ist eine offene Betriebsentscheidung.
- **Netz-/Einbettungs-Topologie** (in-Prozess über die C-ABI, oder der Daemon über gRPC).

---

## Anhang A — Abgrenzung zu lakearch

Geteilte Haltung: genau ein Substrat-Vertrag nach oben; **wertet/rechnet/sortiert nicht**; die Schicht darüber entscheidet. Unterschiede: **veränderbar statt append-only** (§1.2/§6); **pfad- statt inhaltsadressiert** (§3); **klare, menschlich navigierbare Hierarchie statt wildem Hash-Graphen** (§5); **keine automatische Deduplizierung** (§3.4); **Pflicht-Beschreibung + Leitfaden** als aktive Besonderheit (§4/§9). scheme ist damit das Werkzeug für Daten, die *sauber abgelegt und klar benannt* gehören; lakearch das für ein append-only, föderierbares Herkunfts-Substrat.

## Anhang B — Wesen in einem Satz

scheme ist ein **veränderbares** Substrat aus genau einer Entität — dem Knoten (Ordner oder Datei) —, das Daten in einer **klaren, menschlich navigierbaren, pfadadressierten Hierarchie** als echten Dateisystembaum-als-Wahrheit **speichert, liest, verändert und traversiert**, dabei jede Ablage mit einer **erzwungenen Beschreibung** versieht und dem schreibenden Agenten **mitteilt, dass Struktur und Beschreibung Pflicht sind**, während **Rechnen, Werten und Sortieren — und die Wahl von Ort und Beschreibung — bewusst außerhalb** liegen.
