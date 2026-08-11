// brooks-cli, Copyright 2026, Will Hawkins
//
// This file is part of brooks-cli.
//
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

use std::{
    collections::HashMap,
    io,
    path::Path,
    sync::{Arc, Mutex},
};

use actix_web::{App, HttpServer, delete, get, put, web};
use brooks_lib::{cdni::spec::TypedHostMetadata, integrations};
use chrono::{DateTime, Utc, Duration};
use clio::ClioPath;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
};

pub type Hmds = (DateTime<Utc>, TypedHostMetadata<()>);

#[derive(Debug, Clone)]
pub struct HmdsConfiguration {
    hmds: Arc<Mutex<HashMap<String, Hmds>>>,
    server_path: ClioPath,
    timeout: Duration,
}

#[delete("/update/{address}")]
async fn delete_hmd(
    path: web::Path<String>,
    data: web::Data<HmdsConfiguration>,
) -> actix_web::Result<String> {
    let address = path.into_inner();

    let mut hmds = data
        .hmds
        .lock()
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    hmds.remove(&address);

    println!("hmds: {:?}", hmds.keys());

    Ok(format!("Going to remove address {address}"))
}

#[put("/update/{address}")]
async fn add_hmd(
    info: web::Json<TypedHostMetadata<()>>,
    path: web::Path<String>,
    data: web::Data<HmdsConfiguration>,
) -> actix_web::Result<String> {
    let address = path.into_inner();

    let mut hmds = data
        .hmds
        .lock()
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    let expiry = Utc::now() + data.timeout;

    hmds.insert(address.clone(), (expiry, info.0));

    Ok(format!(
        "Adding address {address} to expire after {}, at {expiry}.",
        data.timeout
    ))
}

#[get("/status/")]
async fn status(data: web::Data<HmdsConfiguration>) -> actix_web::Result<String> {
    Ok(format!(
        "Going to get the status: {:?}",
        data.hmds
            .lock()
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?
            .keys()
    ))
}

pub async fn server(ip: String, port: u16, path: ClioPath, timeout: Duration) -> io::Result<()> {
    let metadata: HashMap<String, (DateTime<Utc>, TypedHostMetadata<()>)> = HashMap::new();
    let metadata = Arc::new(Mutex::new(metadata));

    let web_ = HmdsWebConfiguration {
        hmds: metadata.clone(),
        timeout,
    };

    let domain_ = HmdsDomainConfiguration {
        hmds: metadata.clone(),
        server_path: path.clone(),
    };

    let http_configuration = web::Data::new(web_.clone());

    let result = tokio::spawn(async move {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                Ok(())
            },
            result = socket_proxy_server(domain_) => result,
            result = HttpServer::new(move || {
            App::new()
                .app_data(http_configuration.clone())
                .wrap(actix_cors::Cors::permissive())
                .service(delete_hmd)
                .service(add_hmd)
                .service(status)
        })
        .bind((ip, port))?
        .run() => result,
        }
    })
    .await;

    // Instead of using a Drop implementation (because that does not let us show an error),
    // delete the path to the domain socket here.
    fs::remove_file(path.path())?;

    result?
}

async fn write_entire(s: &mut UnixStream, d: &[u8]) -> io::Result<usize> {
    s.write_u64_le(d.len() as u64).await?;

    let mut already_sent = 0usize;
    loop {
        s.writable().await?;
        match s.try_write(&d[already_sent..]) {
            Ok(n) => {
                if n == 0 {
                    return Ok(already_sent);
                }
                already_sent += n;
                if already_sent == d.len() {
                    return Ok(already_sent);
                }
                continue;
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}

async fn read_entire(s: &mut UnixStream, d: &mut [u8]) -> io::Result<usize> {
    let mut already_read = 0usize;
    loop {
        s.readable().await?;
        match s.try_read(&mut d[already_read..]) {
            Ok(n) => {
                if n == 0 {
                    return Ok(already_read);
                }
                already_read += n;
                if already_read == d.len() {
                    return Ok(already_read);
                }
                continue;
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}

pub async fn socket_proxy_server(config: HmdsDomainConfiguration) -> io::Result<()> {
    let server_path = &config.server_path;
    let socket = UnixListener::bind(server_path.path())?;

    loop {
        // Wait for the socket to be readable
        let (mut client, _) = socket.accept().await?;

        client.readable().await?;

        let incoming_buffer_size = match client.read_u64_le().await {
            Ok(ibs) => ibs,
            Err(_) => todo!(),
        };

        let mut buf: Vec<u8> = vec![0; incoming_buffer_size as usize];

        if read_entire(&mut client, &mut buf).await.is_err() {
            todo!();
        };

        let query = String::from_utf8(buf).expect("Could not convert into string");

        let found = {
            let hmds = config.hmds.lock().unwrap();
            hmds.get(&query).cloned()
        };

        match found {
            Some((timestamp, found)) => {
                write_entire(
                    &mut client,
                    serde_json::to_string(&integrations::hmds::ExpirableJsonValue {
                        expiry: timestamp,
                        value: serde_json::to_value(found)?,
                    })
                    .expect("Could not serialize")
                    .as_bytes(),
                )
                .await?;
            }
            None => {
                write_entire(&mut client, &[]).await?;
            }
        }
    }
}

#[allow(unused)]
pub async fn query_proxy_config(query: &str, server_path: &Path) -> io::Result<String> {
    let mut socket = UnixStream::connect(server_path).await?;

    // Wait for the socket to be readable
    socket.writable().await?;

    write_entire(&mut socket, query.as_bytes()).await?;

    socket.readable().await?;

    let incoming_buffer_size = match socket.read_u64_le().await {
        Ok(ibs) => ibs,
        Err(_) => todo!(),
    };

    let mut buf: Vec<u8> = vec![0; incoming_buffer_size as usize];

    if read_entire(&mut socket, &mut buf).await.is_err() {
        todo!();
    };

    let s = String::from_utf8(buf).map_err(|_| io::ErrorKind::InvalidData)?;
    Ok(s)
}
