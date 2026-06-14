use crate::request::Request;
use base64::engine::general_purpose::STANDARD;
use base64::write::EncoderWriter;
use http_body::{Body as _, Empty};
use hyper::body::{Buf, Bytes};
use hyper::client::HttpConnector;
use hyper::header::LOCATION;
use hyper::{Body, Client};
use hyper_tls::native_tls::TlsConnector;
use hyper_tls::HttpsConnector;
use lambda_runtime::{run, service_fn, LambdaEvent};
use serde::Serialize;
use tokio::pin;
use std::io;
use url::Url;

mod request;

const CHUNK_SIZE: usize = 4 * 1024*1024;

type C = Client<HttpsConnector<HttpConnector>, Empty<Bytes>>;
type HttpResponse = hyper::Response<Body>;

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    let mut http = HttpConnector::new();
    http.enforce_http(false);
    let tls = TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()
        .unwrap();
    let https = HttpsConnector::from((http, tls.into()));
    let client = Client::builder().build(https);
    run(service_fn(|e| handler(&client, e))).await
}

async fn handler(
    client: &C,
    event: LambdaEvent<Request>,
) -> Result<Response, lambda_runtime::Error> {
    let (request, _) = event.into_parts();
    let response = fetch(client, &request).await?;
    let status = response.status().as_u16();
    let body = response.into_body();
    pin!(body);
    let mut skip = request.offset.unwrap_or_default();
    let mut take = CHUNK_SIZE;
    let mut writer = EncoderWriter::new(Vec::new(), &STANDARD);
    let mut has_more = false;
    while let Some(buf) = body.data().await {
        let mut buf = buf?;
        if take == 0 && buf.has_remaining() {
            has_more = true;
            break;
        }
        if skip >= buf.remaining() {
            skip -= buf.remaining();
            continue;
        }
        if skip > 0 {
            buf.advance(skip);
            skip = 0;
        }
        if buf.remaining() > take {
            buf.truncate(take);
        }
        take -= buf.remaining();
        let mut reader = buf.reader();
        io::copy(&mut reader, &mut writer)?;
    }
    let data = unsafe { String::from_utf8_unchecked(writer.finish()?) };
    Ok(Response {
        status,
        data,
        next: if has_more { Some(request.offset.unwrap_or_default() + CHUNK_SIZE) } else { None },
    })
}

async fn fetch(client: &C, request: &Request) -> Result<HttpResponse, lambda_runtime::Error> {
    let mut url = Url::parse(&request.uri)?;
    for i in 0.. {
        let mut builder = hyper::Request::builder()
            .method(request.method.clone())
            .uri(url.to_string());
        *builder.headers_mut().unwrap() = request.headers.clone();
        let http_request = builder.body(Empty::<Bytes>::new())?;

        let response = client.request(http_request).await?;
        if i < 10 && response.status().is_redirection() {
            if let Some(location) = response.headers().get(LOCATION) {
                url = url.join(location.to_str()?)?;
                continue;
            }
        }
        return Ok(response);
    }
    unreachable!();
}

#[derive(Serialize)]
pub struct Response {
    pub status: u16,
    pub data: String,
    pub next: Option<usize>,
}
