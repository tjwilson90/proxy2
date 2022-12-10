use base64::engine::DEFAULT_ENGINE;
use base64::write::EncoderWriter;
use http_body::Empty;
use hyper::body::{aggregate, Buf, Bytes};
use hyper::header::{HeaderName, HeaderValue};
use hyper::{Client, HeaderMap, Method, Uri};
use hyper_tls::HttpsConnector;
use lambda_runtime::{run, service_fn, LambdaEvent};
use serde::de::MapAccess;
use serde::{de, Deserialize, Deserializer};
use std::str::FromStr;
use std::{fmt, io};

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    run(service_fn(handler)).await?;
    Ok(())
}

async fn handler(event: LambdaEvent<Request>) -> Result<String, lambda_runtime::Error> {
    let (req, _) = event.into_parts();

    let mut builder = hyper::Request::builder().method(req.method).uri(req.uri);
    *builder.headers_mut().unwrap() = req.headers;
    let request = builder.body(Empty::<Bytes>::new())?;

    let client = Client::builder().build(HttpsConnector::new());
    let resp = client.request(request).await?;
    let bytes = aggregate(resp.into_body()).await?;
    let cap = (2 + 4 * bytes.remaining()) / 3;
    let mut reader = bytes.reader();
    let mut writer = EncoderWriter::from(Vec::with_capacity(cap), &DEFAULT_ENGINE);
    io::copy(&mut reader, &mut writer)?;
    Ok(unsafe { String::from_utf8_unchecked(writer.into_inner()) })
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
