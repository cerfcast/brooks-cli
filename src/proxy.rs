// brooks-cli, Copyright 2026, Will Hawkins
//
// This file is part of brooks-cli.

// This file is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use std::str::FromStr;

use actix_utils::future::ok;
use actix_web::dev::ServiceRequest;
use actix_web::error::{ErrorBadGateway, ErrorInternalServerError};
use actix_web::get;
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::http::uri::{Authority, PathAndQuery};
use actix_web::{HttpRequest, dev::PeerAddr};

use brooks_lib::ps::interpret::{
    ProcessableRequestResponse, ProcessableRequestResponseError, interpret_stage,
};
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

        println!("req: {:?}", req);
        let fut = self.service.call(req);

        {
            let crs = self.crs.clone();
            Box::pin(async move {
                let mut res = fut.await?;

                let mut ars = ActixServiceResponse(&mut res);
                let _ = interpret_stage(
                    &crs,
                    &mut ars,
                    brooks_lib::ps::interpret::PsInterpretMode::Response,
                );

                println!("I am able to handle the response now!");
                Ok(res)
            })
        }
    }
}

#[derive(Debug)]
struct ActixServiceResponse<'a>(&'a mut ServiceResponse);

impl<'a> ProcessableRequestResponse for ActixServiceResponse<'a> {
    fn header_value(&self) -> Option<&str> {
        None
    }

    fn headers(&self) -> &[&str] {
        todo!()
    }

    fn set_header_value(
        &mut self,
        header: &str,
        value: &str,
    ) -> Result<(), ProcessableRequestResponseError> {
        self.0.headers_mut().insert(
            HeaderName::from_str(header).expect("Cannot convert header name"),
            HeaderValue::from_str(value).expect("Cannot convert header value"),
        );
        Ok(())
    }

    fn remove_header(&mut self, header: &str) -> Result<(), ProcessableRequestResponseError> {
        self.0.headers_mut().remove(header);
        Ok(())
    }

    fn add_header(
        &mut self,
        header: &str,
        value: &str,
    ) -> Result<(), ProcessableRequestResponseError> {
        self.0.headers_mut().append(
            HeaderName::from_str(header).map_err(|_| ProcessableRequestResponseError::BadValue)?,
            HeaderValue::from_str(value).map_err(|_| ProcessableRequestResponseError::BadValue)?,
        );
        Ok(())
    }
    fn uri(&self) -> Uri {
        println!("uri: {}", self.0.request().uri());
        http::Uri::builder()
            .scheme(self.0.request().uri().scheme_str().unwrap_or("https"))
            .path_and_query(
                self.0
                    .request()
                    .uri()
                    .path_and_query()
                    .unwrap_or(&PathAndQuery::from_static(""))
                    .as_str(),
            )
            .authority(
                self.0
                    .request()
                    .uri()
                    .authority()
                    .unwrap_or(&Authority::from_static("127.0.0.1"))
                    .to_string(),
            )
            .build()
            .expect("Could not recreate Uri")
    }

    fn set_uri(&mut self, _uri: &Uri) -> Result<(), ProcessableRequestResponseError> {
        Err(ProcessableRequestResponseError::InvalidMode)
    }

    fn set_response(&mut self, response: &u16) -> Result<(), ProcessableRequestResponseError> {
        let sc = actix_web::http::StatusCode::from_u16(*response)
            .map_err(|_| ProcessableRequestResponseError::BadValue)?;
        self.0.response_mut().head_mut().status = sc;
        Ok(())
    }
}

#[derive(Debug)]
struct ActixServiceRequest<'a>(&'a mut ServiceRequest);

impl<'a> ProcessableRequestResponse for ActixServiceRequest<'a> {
    fn header_value(&self) -> Option<&str> {
        todo!()
    }

    fn headers(&self) -> &[&str] {
        todo!()
    }

    fn set_header_value(
        &mut self,
        header: &str,
        value: &str,
    ) -> Result<(), ProcessableRequestResponseError> {
        self.0.headers_mut().insert(
            HeaderName::from_str(header).expect("Cannot convert header name"),
            HeaderValue::from_str(value).expect("Cannot convert header value"),
        );
        Ok(())
    }

    fn remove_header(&mut self, header: &str) -> Result<(), ProcessableRequestResponseError> {
        self.0.headers_mut().remove(header);
        Ok(())
    }

    fn add_header(
        &mut self,
        header: &str,
        value: &str,
    ) -> Result<(), ProcessableRequestResponseError> {
        self.0.headers_mut().append(
            HeaderName::from_str(header).map_err(|_| ProcessableRequestResponseError::BadValue)?,
            HeaderValue::from_str(value).map_err(|_| ProcessableRequestResponseError::BadValue)?,
        );
        Ok(())
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

    fn set_uri(&mut self, _uri: &Uri) -> Result<(), ProcessableRequestResponseError> {
        self.0.head_mut().uri = actix_web::http::Uri::from_str(&_uri.to_string())
            .map_err(|_| ProcessableRequestResponseError::BadValue)?;
        Ok(())
    }

    fn set_response(&mut self, _response: &u16) -> Result<(), ProcessableRequestResponseError> {
        Err(ProcessableRequestResponseError::InvalidMode)
    }
}

fn client_added_header(hn: &str) -> bool {
    hn == "host" || hn == "content-length" || hn == "accept-encoding" || hn == "date"
}

#[get("/proxy/")]
async fn index(req: HttpRequest, _peer: PeerAddr) -> actix_web::Result<String> {
    let query_proxy_url = req.query_string();
    println!("proxy_url: {query_proxy_url}");
    let proxied_url = if !query_proxy_url.is_empty() {
        &actix_web::http::Uri::from_str(req.query_string()).map_err(ErrorBadGateway)?
    } else {
        req.uri()
    };
    let client = awc::Client::default();
    let mut request = client.get(proxied_url);

    // Use any additional headers from the original query, except for the ones
    // that are going to be set by the client (e.g., host).
    for header in req.headers() {
        if !client_added_header(header.0.as_str()) {
            request = request.insert_header_if_none(header);
        }
    }

    let mut response = request.send().await.map_err(ErrorBadGateway)?;
    String::from_utf8(response.body().await.map_err(ErrorBadGateway)?.to_vec())
        .map_err(ErrorBadGateway)
}

pub async fn proxy(
    ip: String,
    port: u16,
    crs: TypedStage<PsVerificationKey>,
) -> std::io::Result<()> {
    use actix_web::{App, HttpServer};

    println!("Proxying on {}:{}", ip, port);
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
