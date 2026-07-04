//! Die **gRPC-Service-Implementierung** des Daemons — der Netz-Rand, der die
//! veränderbaren, pfadbasierten scheme-Primitive (§6/§7) bedient.
//!
//! Maßgeblich: `semantics/scheme.md`. Der Rand reicht **nur** durch: er
//! sortiert/aggregiert/rankt **nichts** (§1.4). Pfade und Beschreibungen werden am
//! Rand validiert (Client-Formfehler ⇒ `InvalidArgument`), die eigentliche
//! (blockierende) Arbeit läuft über den geteilten [`Bestand`].

use tonic::{Request, Response, Status};

use scheme_core::{Beschreibung, Pfad};

use crate::bestand::{status_from, Bestand};
use crate::proto::scheme_server::Scheme;
use crate::proto::{
    DeleteRequest, DeleteResponse, DescribeRequest, DescribeResponse, Empty, GetRequest,
    GetResponse, ListRequest, ListResponse, PutRequest, PutResponse, PutStructuredRequest,
    RenameRequest, SetDescriptionRequest, UpdateRequest,
};

/// Die gRPC-Service-Implementierung über **einem** [`Bestand`].
pub struct SchemeService {
    bestand: Bestand,
}

impl SchemeService {
    /// Baut den Service über einem geöffneten [`Bestand`].
    pub fn new(bestand: Bestand) -> Self {
        SchemeService { bestand }
    }
}

#[tonic::async_trait]
impl Scheme for SchemeService {
    async fn put_structured(
        &self,
        req: Request<PutStructuredRequest>,
    ) -> Result<Response<PutResponse>, Status> {
        let r = req.into_inner();
        let pfad = Pfad::parse(&r.path).map_err(status_from)?;
        let beschr = Beschreibung::neu(r.description).map_err(status_from)?;
        let data = r.data;
        let knoten = self.bestand.run(move |b| b.ablegen(&pfad, beschr, &data)).await?;
        Ok(Response::new(PutResponse {
            path: knoten.pfad.als_str().to_string(),
        }))
    }

    async fn put(&self, req: Request<PutRequest>) -> Result<Response<PutResponse>, Status> {
        let r = req.into_inner();
        let pfad_opt = if r.path.is_empty() {
            None
        } else {
            Some(Pfad::parse(&r.path).map_err(status_from)?)
        };
        let data = r.data;
        let knoten = self
            .bestand
            .run(move |b| b.ablegen_roh(pfad_opt.as_ref(), &data))
            .await?;
        Ok(Response::new(PutResponse {
            path: knoten.pfad.als_str().to_string(),
        }))
    }

    async fn get(&self, req: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let r = req.into_inner();
        let pfad = Pfad::parse(&r.path).map_err(status_from)?;
        let bytes = self.bestand.run(move |b| b.lesen(&pfad)).await?;
        Ok(Response::new(match bytes {
            Some(d) => GetResponse {
                present: true,
                data: d,
            },
            None => GetResponse {
                present: false,
                data: Vec::new(),
            },
        }))
    }

    async fn update(&self, req: Request<UpdateRequest>) -> Result<Response<Empty>, Status> {
        let r = req.into_inner();
        let pfad = Pfad::parse(&r.path).map_err(status_from)?;
        let data = r.data;
        self.bestand.run(move |b| b.aktualisieren(&pfad, &data)).await?;
        Ok(Response::new(Empty {}))
    }

    async fn rename(&self, req: Request<RenameRequest>) -> Result<Response<Empty>, Status> {
        let r = req.into_inner();
        let von = Pfad::parse(&r.from).map_err(status_from)?;
        let nach = Pfad::parse(&r.to).map_err(status_from)?;
        self.bestand.run(move |b| b.verschieben(&von, &nach)).await?;
        Ok(Response::new(Empty {}))
    }

    async fn delete(
        &self,
        req: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let r = req.into_inner();
        let pfad = Pfad::parse(&r.path).map_err(status_from)?;
        let removed = self.bestand.run(move |b| b.loeschen(&pfad)).await?;
        Ok(Response::new(DeleteResponse { removed }))
    }

    async fn set_description(
        &self,
        req: Request<SetDescriptionRequest>,
    ) -> Result<Response<Empty>, Status> {
        let r = req.into_inner();
        let pfad = Pfad::parse(&r.path).map_err(status_from)?;
        let beschr = Beschreibung::neu(r.description).map_err(status_from)?;
        self.bestand.run(move |b| b.beschreiben(&pfad, beschr)).await?;
        Ok(Response::new(Empty {}))
    }

    async fn list(&self, req: Request<ListRequest>) -> Result<Response<ListResponse>, Status> {
        let r = req.into_inner();
        let prefix = if r.prefix.is_empty() {
            Pfad::wurzel()
        } else {
            Pfad::parse(&r.prefix).map_err(status_from)?
        };
        let pfade = self.bestand.run(move |b| b.auflisten(&prefix)).await?;
        Ok(Response::new(ListResponse {
            paths: pfade.iter().map(|p| p.als_str().to_string()).collect(),
        }))
    }

    async fn describe(
        &self,
        _req: Request<DescribeRequest>,
    ) -> Result<Response<DescribeResponse>, Status> {
        Ok(Response::new(DescribeResponse {
            guidance: self.bestand.leitfaden().to_string(),
        }))
    }
}
