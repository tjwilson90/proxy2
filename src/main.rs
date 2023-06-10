use base64::engine::general_purpose::STANDARD;
use base64::write::EncoderWriter;
use http_body::Empty;
use hyper::body::{aggregate, Buf, Bytes};
use hyper::client::HttpConnector;
use hyper::header::{HeaderName, HeaderValue, LOCATION};
use hyper::{Body, Client, HeaderMap, Method, Response, Uri};
use hyper_tls::native_tls::TlsConnector;
use hyper_tls::HttpsConnector;
use lambda_runtime::{run, service_fn, LambdaEvent};
use serde::de::MapAccess;
use serde::{de, Deserialize, Deserializer};
use std::str::FromStr;
use std::{fmt, io};

type C = Client<HttpsConnector<HttpConnector>, Empty<Bytes>>;

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

async fn handler(client: &C, event: LambdaEvent<Request>) -> Result<String, lambda_runtime::Error> {
    let (request, _) = event.into_parts();
    let response = fetch(client, request.method, request.uri, request.headers).await?;
    let bytes = aggregate(response.into_body()).await?;
    let mut writer = EncoderWriter::new(
        Vec::with_capacity(base64::encoded_len(bytes.remaining(), true).unwrap()),
        &STANDARD,
    );
    let mut reader = bytes.reader();
    io::copy(&mut reader, &mut writer)?;
    Ok(unsafe { String::from_utf8_unchecked(writer.finish()?) })
}

async fn fetch(
    client: &C,
    method: Method,
    mut uri: Uri,
    headers: HeaderMap,
) -> Result<Response<Body>, lambda_runtime::Error> {
    for i in 0.. {
        let mut builder = hyper::Request::builder().method(method.clone()).uri(uri);
        *builder.headers_mut().unwrap() = headers.clone();
        let request = builder.body(Empty::<Bytes>::new())?;

        let response = client.request(request).await?;
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

#[derive(Deserialize)]
struct Request {
    #[serde(deserialize_with = "method")]
    method: Method,
    #[serde(deserialize_with = "uri")]
    uri: Uri,
    #[serde(default, deserialize_with = "headers")]
    headers: HeaderMap,
}

fn method<'de, D: Deserializer<'de>>(deser: D) -> Result<Method, D::Error> {
    struct V;
    impl<'de> de::Visitor<'de> for V {
        type Value = Method;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an http method")
        }

        fn visit_borrowed_str<E: de::Error>(self, val: &'de str) -> Result<Self::Value, E> {
            Method::from_str(val).map_err(de::Error::custom)
        }

        fn visit_string<E: de::Error>(self, val: String) -> Result<Self::Value, E> {
            self.visit_borrowed_str(&val)
        }
    }
    deser.deserialize_str(V)
}

fn uri<'de, D: Deserializer<'de>>(deser: D) -> Result<Uri, D::Error> {
    struct V;
    impl<'de> de::Visitor<'de> for V {
        type Value = Uri;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an http uri")
        }

        fn visit_borrowed_str<E: de::Error>(self, val: &'de str) -> Result<Self::Value, E> {
            Uri::from_str(val).map_err(de::Error::custom)
        }

        fn visit_string<E: de::Error>(self, val: String) -> Result<Self::Value, E> {
            self.visit_borrowed_str(&val)
        }
    }
    deser.deserialize_str(V)
}

fn headers<'de, D: Deserializer<'de>>(deser: D) -> Result<HeaderMap, D::Error> {
    struct V;
    impl<'de> de::Visitor<'de> for V {
        type Value = HeaderMap;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("http headers")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut headers = HeaderMap::with_capacity(map.size_hint().unwrap_or(0));
            while let Some((key, val)) = map.next_entry::<&'de str, &'de str>()? {
                let key = HeaderName::from_str(key).map_err(de::Error::custom)?;
                let val = HeaderValue::from_str(val).map_err(de::Error::custom)?;
                headers.insert(key, val);
            }
            Ok(headers)
        }
    }
    deser.deserialize_map(V)
}
