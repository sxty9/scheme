//! Ende-zu-Ende-Integrationstest des Daemons: startet den gRPC-Server **in-process**
//! auf einem **ephemeren** Port (`127.0.0.1:0`) und fährt einen echten tonic-Client
//! dagegen. Deckt den vollen veränderbaren Lebenszyklus ab (§6/§7/§9): strukturiert
//! ablegen → lesen; Pflicht-Beschreibung (§4); aktualisieren; verschieben; löschen
//! (idempotent); auflisten; und den Leitfaden (§9).

use schemed::proto::scheme_client::SchemeClient;
use schemed::proto::{
    DeleteRequest, DescribeRequest, GetRequest, ListRequest, PutRequest, PutStructuredRequest,
    RenameRequest, UpdateRequest,
};
use schemed::proto::scheme_server::SchemeServer;
use schemed::{Bestand, SchemeService};

use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Channel, Server};
use tonic::Code;

/// Startet den Daemon in-process auf einem ephemeren Port und liefert die
/// verbundene Client-Stub + das Temp-Verzeichnis (dessen Drop den Bestand entfernt).
async fn start_server() -> (SchemeClient<Channel>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let bestand = Bestand::open(dir.path()).expect("bestand öffnen");
    let service = SchemeService::new(bestand);

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind :0");
    let addr = listener.local_addr().expect("local_addr");
    let incoming = TcpListenerStream::new(listener);

    tokio::spawn(async move {
        Server::builder()
            .add_service(SchemeServer::new(service))
            .serve_with_incoming(incoming)
            .await
            .expect("server lief");
    });

    let endpoint = format!("http://{addr}");
    let mut last_err = None;
    for _ in 0..50 {
        match SchemeClient::connect(endpoint.clone()).await {
            Ok(client) => return (client, dir),
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        }
    }
    panic!("Client konnte nicht verbinden: {last_err:?}");
}

#[tokio::test]
async fn voller_lebenszyklus_ueber_grpc() {
    let (mut c, _dir) = start_server().await;

    // strukturiert ablegen → lesen
    let put = c
        .put_structured(PutStructuredRequest {
            path: "raum/hardware/computer/anleitung.pdf".into(),
            description: "Bedienungsanleitung des Computers".into(),
            data: b"PDF".to_vec(),
        })
        .await
        .expect("put_structured")
        .into_inner();
    assert_eq!(put.path, "raum/hardware/computer/anleitung.pdf");

    let got = c
        .get(GetRequest {
            path: "raum/hardware/computer/anleitung.pdf".into(),
        })
        .await
        .expect("get")
        .into_inner();
    assert!(got.present);
    assert_eq!(got.data, b"PDF");

    // Pflicht-Beschreibung (§4): leere Beschreibung ⇒ InvalidArgument
    let err = c
        .put_structured(PutStructuredRequest {
            path: "raum/x.txt".into(),
            description: "   ".into(),
            data: b"y".to_vec(),
        })
        .await
        .expect_err("leere Beschreibung muss abgelehnt werden");
    assert_eq!(err.code(), Code::InvalidArgument);

    // Basis-Put (roh) mit Pfad überschreibt; ohne Pfad ⇒ eingang/
    c.put(PutRequest {
        path: "raum/notiz.txt".into(),
        data: b"v1".to_vec(),
    })
    .await
    .expect("put");
    c.update(UpdateRequest {
        path: "raum/notiz.txt".into(),
        data: b"v2".to_vec(),
    })
    .await
    .expect("update");
    let notiz = c
        .get(GetRequest {
            path: "raum/notiz.txt".into(),
        })
        .await
        .expect("get notiz")
        .into_inner();
    assert_eq!(notiz.data, b"v2");

    let eingang = c
        .put(PutRequest {
            path: String::new(),
            data: b"lose".to_vec(),
        })
        .await
        .expect("put eingang")
        .into_inner();
    assert!(eingang.path.starts_with("eingang/"), "landete: {}", eingang.path);

    // auflisten
    let liste = c
        .list(ListRequest {
            prefix: "raum".into(),
        })
        .await
        .expect("list")
        .into_inner();
    assert!(liste.paths.contains(&"raum/hardware/computer/anleitung.pdf".to_string()));
    assert!(liste.paths.contains(&"raum/notiz.txt".to_string()));

    // verschieben
    c.rename(RenameRequest {
        from: "raum/notiz.txt".into(),
        to: "archiv/notiz.txt".into(),
    })
    .await
    .expect("rename");
    assert!(
        !c.get(GetRequest {
            path: "raum/notiz.txt".into()
        })
        .await
        .unwrap()
        .into_inner()
        .present
    );
    assert_eq!(
        c.get(GetRequest {
            path: "archiv/notiz.txt".into()
        })
        .await
        .unwrap()
        .into_inner()
        .data,
        b"v2"
    );

    // löschen (idempotent)
    let d1 = c
        .delete(DeleteRequest {
            path: "archiv/notiz.txt".into(),
        })
        .await
        .expect("delete")
        .into_inner();
    assert!(d1.removed);
    let d2 = c
        .delete(DeleteRequest {
            path: "archiv/notiz.txt".into(),
        })
        .await
        .expect("delete idempotent")
        .into_inner();
    assert!(!d2.removed);

    // Leitfaden (§9)
    let guidance = c
        .describe(DescribeRequest {})
        .await
        .expect("describe")
        .into_inner()
        .guidance;
    assert!(guidance.contains("klar strukturiert"));
    assert!(guidance.contains("Beschreibung"));
}
