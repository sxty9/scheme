//! `schemed` — der Daemon-Einstiegspunkt.
//!
//! Öffnet **einen** Bestand (ein [`scheme_core::Bestand`]) und bedient die
//! veränderbaren, pfadbasierten scheme-Primitive (§6/§7) über gRPC (tonic).
//!
//! Konfiguration über Umgebung/Argumente, bewusst minimal:
//! - `arg[1]` (optional): Bestand-Verzeichnis (Default `./scheme-data`).
//! - `SCHEMED_ADDR` (optional): Bind-Adresse (Default `127.0.0.1:50052`).

use std::net::SocketAddr;

use schemed::proto::scheme_server::SchemeServer;
use schemed::{Bestand, SchemeService};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "./scheme-data".to_string());
    std::fs::create_dir_all(&dir)?;

    let addr: SocketAddr = std::env::var("SCHEMED_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:50052".to_string())
        .parse()?;

    // Ein Bestand = ein veränderbarer Dateisystembaum-als-Wahrheit (§5/§6).
    let bestand = Bestand::open(&dir)?;
    let service = SchemeService::new(bestand);

    tracing::info!(%addr, %dir, "schemed startet (ein Bestand, veränderbarer Baum §5/§6)");

    Server::builder()
        .add_service(SchemeServer::new(service))
        .serve_with_shutdown(addr, async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("schemed fährt herunter (SIGINT)");
        })
        .await?;

    Ok(())
}
