use std::net::IpAddr;
use std::sync::Arc;

use actix_web::{HttpRequest, dev::PeerAddr, http::uri::Scheme, post, web};

use brooks_lib::logging::{LogLevel::Trace, LogMsgs};
use brooks_lib::mel::{
    analysis,
    compiler::compile,
    interpreter::{
        self,
        builtins::{BooleanBuiltin, BuiltinFunction, Path_ElementBuiltin},
        interpret::{MelInterpContext, StructValue, TypedValue, Value},
    },
    scope::Scopes,
    tvs::{Struct, Type},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Mel {
    pub expr: String,
}

#[derive(Serialize)]
struct MelResponse {
    pub value: String,
    pub log: LogMsgs,
}

fn header_type_from_req(value: &HttpRequest) -> Struct {
    // Make the header type.
    let mut ht = Struct::new("h");
    value.headers().iter().for_each(|header| {
        ht.insert_field(
            &header.0.to_string().replace("-", "_").to_lowercase(),
            Type::String,
        );
    });
    ht
}

fn uri_type() -> Struct {
    // Make the URI type.
    let mut urit = Struct::new("uri");
    urit.insert_field("path", Type::String);
    urit.insert_field("query", Type::String);

    urit
}

fn req_type(header_type: Struct, uri_type: Struct) -> Struct {
    // Make the req type.
    let mut reqs = Struct::new("req");
    reqs.insert_field("h", Type::Struct(header_type));
    reqs.insert_field("uri", Type::Struct(uri_type));
    reqs.insert_field("method", Type::String);
    reqs.insert_field("scheme", Type::String);
    reqs.insert_field("clientip", Type::IPAddress);
    reqs.insert_field("clientport", Type::Integer);

    reqs
}

fn type_scope_from_req(value: &HttpRequest) -> Scopes<Type> {
    // Set up the built-in variables for type checking.
    let mut scope = Scopes::<Type>::default();

    let ht = header_type_from_req(value);
    let urit = uri_type();
    let reqs = req_type(ht, urit);

    // Add those types to the scope.
    scope = scope.insert("req", Type::Struct(reqs));

    scope
}

fn value_scope_from_req(
    value: &HttpRequest,
    clientip: &IpAddr,
    clientport: u16,
) -> Scopes<TypedValue> {
    let ht = header_type_from_req(value);
    let urit = uri_type();
    let reqt = req_type(ht.clone(), urit.clone());

    // Set up the built-in variables for interpreting.
    let mut value_scope = Scopes::<TypedValue>::default();

    let mut reqv = StructValue::new(reqt.clone());

    let mut hv = StructValue::new(ht.clone());

    value.headers().iter().for_each(|header| {
        if let Ok(x) = header.1.to_str() {
            hv.insert_field(
                &header.0.to_string().replace("-", "_").to_lowercase(),
                TypedValue {
                    value: Value::String(x.to_string()),
                    tipe: Type::String,
                },
            )
            .expect("header field value is mistyped");
        }
    });

    let mut uriv = StructValue::new(urit.clone());

    uriv.insert_field(
        "path",
        TypedValue {
            value: Value::String(value.uri().path().to_string()),
            tipe: Type::String,
        },
    )
    .expect("path field value is mistyped.");

    uriv.insert_field(
        "query",
        TypedValue {
            value: Value::String(value.uri().query().unwrap_or_default().to_string()),
            tipe: Type::String,
        },
    )
    .expect("query field value is mistyped.");

    reqv.insert_field(
        "h",
        TypedValue {
            value: Value::Struct(hv),
            tipe: Type::Struct(ht.clone()),
        },
    )
    .expect("h field value is mistyped.");

    reqv.insert_field(
        "uri",
        TypedValue {
            value: Value::Struct(uriv),
            tipe: Type::Struct(urit.clone()),
        },
    )
    .expect("uri field value is mistyped.");

    reqv.insert_field(
        "method",
        TypedValue {
            value: Value::String(value.method().to_string()),
            tipe: Type::String,
        },
    )
    .expect("method field value is mistyped.");

    reqv.insert_field(
        "scheme",
        TypedValue {
            value: Value::String(value.uri().scheme().unwrap_or(&Scheme::HTTP).to_string()),
            tipe: Type::String,
        },
    )
    .expect("Header field value is mistyped.");

    reqv.insert_field(
        "clientip",
        TypedValue {
            value: Value::IPAddress(*clientip),
            tipe: Type::IPAddress,
        },
    )
    .expect("clientip field value is mistyped.");

    reqv.insert_field(
        "clientport",
        TypedValue {
            value: Value::Integer(clientport as i64),
            tipe: Type::Integer,
        },
    )
    .expect("clientport field value is mistyped.");

    value_scope = value_scope.insert(
        "req",
        TypedValue {
            value: Value::Struct(reqv),
            tipe: Type::Struct(reqt),
        },
    );

    value_scope
}

#[post("/{tail:.*}")] // <- define path parameters
async fn index(
    req: HttpRequest,
    payload: web::Json<Mel>,
    peer: PeerAddr,
) -> actix_web::Result<String> {
    println!("Serving request from {peer}");

    let path_element_builtin = Path_ElementBuiltin {};
    let boolean_builtin = BooleanBuiltin {};

    let clientip = peer.0.ip();
    let clientport = peer.0.port();

    // Set up the built-in variables for type checking.
    let analysis_scopes = type_scope_from_req(&req);

    // Set up the built-in variables for interpreting.
    let mut interp_scopes = value_scope_from_req(&req, &clientip, clientport);
    interp_scopes = interp_scopes.insert(
        &path_element_builtin.name(),
        TypedValue {
            value: Value::Function(Arc::new(path_element_builtin.clone())),
            tipe: Type::Function(
                Arc::new(path_element_builtin.return_type()),
                path_element_builtin.parameters(),
            ),
        },
    );
    interp_scopes = interp_scopes.insert(
        &boolean_builtin.name(),
        TypedValue {
            value: Value::Function(Arc::new(boolean_builtin.clone())),
            tipe: Type::Function(
                Arc::new(boolean_builtin.return_type()),
                boolean_builtin.parameters(),
            ),
        },
    );

    let result = match compile(&payload.expr) {
        Ok(expr) => expr,
        Err(e) => {
            return Err(actix_web::error::ErrorBadRequest(std::io::Error::other(
                format!("{:?}", e),
            )));
        }
    };

    let result = analysis::analyze(&result, analysis_scopes)
        .map_err(|e| actix_web::error::ErrorBadRequest(std::io::Error::other(e.to_string())))?;

    let mut interp_context = MelInterpContext::default();
    interp_context = interp_context
        .update_log(LogMsgs::new(Trace))
        .update_scopes(interp_scopes);

    match interpreter::interpret(&result, interp_context) {
        Ok(o) => match o.val {
            Some(val) => {
                let result = MelResponse {
                    value: format!("{}", val),
                    log: o.log,
                };
                Ok(serde_json::to_string(&result).expect("Could not serialize the result"))
            }
            None => Err(actix_web::error::ErrorBadRequest(std::io::Error::other(
                "After successful evaluation, the expression produced no value",
            ))),
        },
        Err(e) => Err(actix_web::error::ErrorBadRequest(std::io::Error::other(
            e.to_string(),
        ))),
    }
}

pub async fn serve(ip: String, port: u16) -> std::io::Result<()> {
    use actix_web::{App, HttpServer};

    println!("Serving on {}:{}", ip, port);
    HttpServer::new(|| {
        App::new()
            .wrap(actix_cors::Cors::permissive())
            .service(index)
    })
    .bind((ip, port))?
    .run()
    .await
}
