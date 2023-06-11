use hyper::header::{HeaderName, HeaderValue};
use hyper::{HeaderMap, Method, Uri};
use serde::de::MapAccess;
use serde::{de, Deserialize, Deserializer};
use std::fmt;
use std::str::FromStr;

#[derive(Deserialize)]
pub struct Request {
    #[serde(deserialize_with = "method")]
    pub method: Method,
    #[serde(deserialize_with = "uri")]
    pub uri: Uri,
    #[serde(default, deserialize_with = "headers")]
    pub headers: HeaderMap,
    pub offset: Option<usize>,
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
