use crate::cache::Cache;
use crate::request::Request;
use base64::engine::general_purpose::STANDARD;
use base64::write::EncoderWriter;
use http_body::Empty;
use hyper::body::{aggregate, Buf, Bytes};
use hyper::client::HttpConnector;
use hyper::header::LOCATION;
use hyper::{Body, Client, Uri};
use hyper_tls::native_tls::TlsConnector;
use hyper_tls::HttpsConnector;
use lambda_runtime::{run, service_fn, LambdaEvent};
use serde::Serialize;
use std::io;

mod cache;
mod request;

const MAX_RESPONSE_LEN: usize = 6291456;

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
    let cache = Cache::new();
    run(service_fn(|e| handler(&client, &cache, e))).await
}

async fn handler(
    client: &C,
    cache: &Cache,
    event: LambdaEvent<Request>,
) -> Result<Response, lambda_runtime::Error> {
    let (request, _) = event.into_parts();
    if let Some(offset) = request.offset {
        if let Some(data) = cache.remove(&request.uri) {
            return Ok(slice_response(request.uri, data, offset, cache));
        }
    }
    let response = fetch(client, &request).await?;
    let bytes = aggregate(response.into_body()).await?;
    let mut writer = EncoderWriter::new(
        Vec::with_capacity(base64::encoded_len(bytes.remaining(), true).unwrap()),
        &STANDARD,
    );
    let mut reader = bytes.reader();
    io::copy(&mut reader, &mut writer)?;
    let data = unsafe { String::from_utf8_unchecked(writer.finish()?) };
    Ok(slice_response(
        request.uri,
        data,
        request.offset.unwrap_or(0),
        cache,
    ))
}

async fn fetch(client: &C, request: &Request) -> Result<HttpResponse, lambda_runtime::Error> {
    let mut uri = request.uri.clone();
    for i in 0.. {
        let mut builder = hyper::Request::builder()
            .method(request.method.clone())
            .uri(uri);
        *builder.headers_mut().unwrap() = request.headers.clone();
        let http_request = builder.body(Empty::<Bytes>::new())?;

        let response = client.request(http_request).await?;
        if i < 10 && response.status().is_redirection() {
            if let Some(location) = response.headers().get(LOCATION) {
                uri = Uri::try_from(location.as_bytes())?;
                continue;
            }
        }
        return Ok(response);
    }
    unreachable!();
}

#[derive(Serialize)]
pub struct Response {
    pub data: String,
    pub next: Option<usize>,
}

fn slice_response(uri: Uri, data: String, offset: usize, cache: &Cache) -> Response {
    if offset + MAX_RESPONSE_LEN < data.len() {
        let slice = data[offset..offset + MAX_RESPONSE_LEN].to_string();
        cache.insert(uri, data);
        Response {
            data: slice,
            next: Some(offset + MAX_RESPONSE_LEN),
        }
    } else {
        Response {
            data: if offset == 0 {
                data
            } else {
                data[offset..].to_string()
            },
            next: None,
        }
    }
}
