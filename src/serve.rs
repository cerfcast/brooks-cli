use std::{collections::HashMap, sync::Arc};

use actix_web::{HttpRequest, dev::PeerAddr, http::uri::Scheme, post, web};
use brooks_lib::{
    analysis,
    interpreter::{
        self, StructValue, TypedValue, Value, builtins::BuiltinFunction,
        builtins::Path_ElementBuiltin,
    },
    scope::Scopes,
    tvs::{Struct, Type},
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Mel {
    pub expr: String,
}

#[post("/{tail:.*}")] // <- define path parameters
async fn index(
    req: HttpRequest,
    payload: web::Json<Mel>,
    peer: PeerAddr,
) -> actix_web::Result<String> {
    let path_element_builtin = Path_ElementBuiltin {};

    let clientip = peer.0.ip();
    let clientport = peer.0.port();

    // Set up the built-in variables for type checking.
    let mut analysis_scopes = Scopes::<Type>::default();

    let mut ht = Struct {
        name: "h".to_string(),
        fields: HashMap::new(),
    };

    req.headers().iter().for_each(|header| {
        ht.fields.insert(
            header.0.to_string().replace("-", "_").to_lowercase(),
            Type::String,
        );
    });

    let mut urit = Struct {
        name: "uri".to_string(),
        fields: HashMap::new(),
    };

    urit.fields.insert("path".to_string(), Type::String);
    urit.fields.insert("query".to_string(), Type::String);

    let mut reqs = Struct {
        name: "req".to_string(),
        fields: HashMap::new(),
    };
    reqs.fields
        .insert("h".to_string(), Type::Struct(ht.clone()));
    reqs.fields
        .insert("uri".to_string(), Type::Struct(urit.clone()));
    reqs.fields.insert("method".to_string(), Type::String);
    reqs.fields.insert("scheme".to_string(), Type::String);
    reqs.fields.insert("clientip".to_string(), Type::IPAddress);
    reqs.fields.insert("clientport".to_string(), Type::Integer);

    analysis_scopes = analysis_scopes.insert("req", Type::Struct(reqs.clone()));

    analysis_scopes = analysis_scopes.insert(
        &path_element_builtin.name(),
        Type::Function(
            Arc::new(path_element_builtin.return_type()),
            path_element_builtin.parameters(),
        ),
    );

    // Set up the built-in variables for interpreting.
    let mut interp_scopes = Scopes::<TypedValue>::default();

    let mut reqsv = StructValue {
        fields: HashMap::new(),
        tpe: reqs.clone(),
    };

    let mut hv = StructValue {
        fields: HashMap::new(),
        tpe: ht.clone(),
    };

    req.headers().iter().for_each(|header| {
        if let Ok(x) = header.1.to_str() {
            hv.fields.insert(
                header.0.to_string().replace("-", "_").to_lowercase(),
                TypedValue {
                    value: Value::String(x.to_string()),
                    tipe: Type::String,
                },
            );
        }
    });

    let mut uriv = StructValue {
        fields: HashMap::new(),
        tpe: urit.clone(),
    };

    uriv.fields.insert(
        "path".to_string(),
        TypedValue {
            value: Value::String(req.uri().path().to_string()),
            tipe: Type::String,
        },
    );
    uriv.fields.insert(
        "query".to_string(),
        TypedValue {
            value: Value::String(req.uri().query().unwrap_or_default().to_string()),
            tipe: Type::String,
        },
    );

    reqsv.fields.insert(
        "h".to_string(),
        TypedValue {
            value: Value::Struct(hv),
            tipe: Type::Struct(ht.clone()),
        },
    );
    reqsv.fields.insert(
        "uri".to_string(),
        TypedValue {
            value: Value::Struct(uriv),
            tipe: Type::Struct(urit.clone()),
        },
    );
    reqsv.fields.insert(
        "method".to_string(),
        TypedValue {
            value: Value::String(req.method().to_string()),
            tipe: Type::String,
        },
    );
    reqsv.fields.insert(
        "scheme".to_string(),
        TypedValue {
            value: Value::String(req.uri().scheme().unwrap_or(&Scheme::HTTP).to_string()),
            tipe: Type::String,
        },
    );

    reqsv.fields.insert(
        "clientip".to_string(),
        TypedValue {
            value: Value::IPAddress(clientip),
            tipe: Type::IPAddress,
        },
    );

    reqsv.fields.insert(
        "clientport".to_string(),
        TypedValue {
            value: Value::Integer(clientport as i64),
            tipe: Type::Integer,
        },
    );

    interp_scopes = interp_scopes.insert(
        "req",
        TypedValue {
            value: Value::Struct(reqsv),
            tipe: Type::Struct(reqs),
        },
    );

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

    let compiled = analysis::compile_and_analyze(&payload.expr, analysis_scopes)
        .map_err(|e| actix_web::error::ErrorBadRequest(std::io::Error::other(e.to_string())))?;
    let value = interpreter::interpret(&compiled, interp_scopes)
        .map_err(|e| actix_web::error::ErrorBadRequest(std::io::Error::other(e.to_string())))?;

    Ok(format!("{}", value))
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
