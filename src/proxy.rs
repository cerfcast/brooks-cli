use std::str::FromStr;

use actix_utils::future::ok;
use actix_web::dev::ServiceRequest;
use actix_web::error::ErrorInternalServerError;
use actix_web::get;
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::http::uri::{Authority, PathAndQuery};
use actix_web::{HttpRequest, dev::PeerAddr};

use brooks_lib::ps::interpret::{ProcessableRequestResponse, interpret_stage};
use brooks_lib::ps::spec::TypedStage;
use brooks_lib::ps::verify::PsVerificationKey;
use futures_util::FutureExt;
use http::Uri;

use std::future::{Ready, ready};

use actix_web::{
    Error,
    dev::{Service, ServiceResponse, Transform, forward_ready},
};
use futures_util::future::LocalBoxFuture;

pub struct ProcessingStagesMiddleware {
    crs: TypedStage<PsVerificationKey>,
}

impl<S> Transform<S, ServiceRequest> for ProcessingStagesMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = Error>,
    S::Future: 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type InitError = ();
    type Transform = ProcessingStagesMiddlewareImpl<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(ProcessingStagesMiddlewareImpl {
            service,
            crs: self.crs.clone(),
        }))
    }
}

pub struct ProcessingStagesMiddlewareImpl<S> {
    service: S,
    crs: TypedStage<PsVerificationKey>,
}

impl<S> Service<ServiceRequest> for ProcessingStagesMiddlewareImpl<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = Error>,
    S::Future: 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        if let Err(e) = interpret_stage(
            &self.crs,
            &mut ActixServiceRequest(&mut req),
            brooks_lib::ps::interpret::PsInterpretMode::Request,
        ) {
            return ok(req.error_response(ErrorInternalServerError(e.to_string()))).boxed_local();
        }
        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            Ok(res)
        })
    }
}

#[derive(Debug)]
struct ActixServiceRequest<'a>(&'a mut ServiceRequest);

impl<'a> ProcessableRequestResponse for ActixServiceRequest<'a> {
    fn header_value(&self) -> &str {
        todo!()
    }

    fn headers(&self) -> &[&str] {
        todo!()
    }

    fn set_header_value(&mut self, header: &str, value: &str) {
        self.0.headers_mut().insert(
            HeaderName::from_str(header).expect("Cannot convert header name"),
            HeaderValue::from_str(value).expect("Cannot convert header value"),
        );
    }

    fn remove_header(&mut self, header: &str) {
        self.0.headers_mut().remove(header);
    }

    fn add_header(&mut self, header: &str, value: &str) {
        self.0.headers_mut().append(
            HeaderName::from_str(header).expect("Cannot convert header name"),
            HeaderValue::from_str(value).expect("Cannot convert header value"),
        );
    }
    fn uri(&self) -> Uri {
        println!("uri: {}", self.0.uri());
        http::Uri::builder()
            .scheme(self.0.uri().scheme_str().unwrap_or("https"))
            .path_and_query(
                self.0
                    .uri()
                    .path_and_query()
                    .unwrap_or(&PathAndQuery::from_static(""))
                    .as_str(),
            )
            .authority(
                self.0
                    .uri()
                    .authority()
                    .unwrap_or(&Authority::from_static("127.0.0.1"))
                    .to_string(),
            )
            .build()
            .expect("Could not recreate Uri")
    }

    fn set_uri(&mut self, _uri: &Uri) {
        todo!()
    }

    fn set_response(&mut self, _response: &i64) {
        todo!()
    }
}

#[get("/")] // <- define path parameters
async fn index(req: HttpRequest, _peer: PeerAddr) -> actix_web::Result<String> {
    println!("proxy_url: {:?}", req.query_string());
    Ok("Done".to_string())
}

pub async fn proxy(
    ip: String,
    port: u16,
    crs: TypedStage<PsVerificationKey>,
) -> std::io::Result<()> {
    use actix_web::{App, HttpServer};

    println!("Serving on {}:{}", ip, port);
    HttpServer::new(move || {
        App::new()
            .wrap(ProcessingStagesMiddleware { crs: crs.clone() })
            .wrap(actix_cors::Cors::permissive())
            .service(index)
    })
    .bind((ip, port))?
    .run()
    .await
}
