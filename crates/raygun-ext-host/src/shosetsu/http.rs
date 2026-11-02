//! HTTP for Shosetsu scripts (M32): the OkHttp-shaped API kotlin-lib (GPL-3.0) gives scripts
//! (`GET`, `POST`, `Request`, `RequestDocument`, `GETDocument`, builders for headers, form bodies
//! and requests), run as blocking calls on the script's worker thread. Scripts may reach any
//! public host, never this machine or the local network: names and redirects are checked, and
//! the resolver drops private answers.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use mlua::{AnyUserData, Lua, Table, UserData, UserDataMethods, Value};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use super::dom::{LuaNode, parse_document};
use crate::cookies::CookieJar;
use crate::permissions::{HostPattern, NetPolicy, is_public_ip};

/// Response bodies larger than this fail the request.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(20);
const MAX_REDIRECTS: usize = 10;

/// Resolves names, keeping only public addresses.
struct PublicResolver;

impl Resolve for PublicResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let found = tokio::net::lookup_host((host.as_str(), 0)).await?;
            let public: Vec<SocketAddr> =
                found.filter(|address| is_public_ip(address.ip())).collect();
            if public.is_empty() {
                return Err(format!("{host} doesn't resolve to a public address").into());
            }
            Ok(Box::new(public.into_iter()) as Addrs)
        })
    }
}

/// Runs a script's requests: its own small runtime, since the script waits for each answer.
pub struct Fetcher {
    client: reqwest::Client,
    runtime: tokio::runtime::Runtime,
    policy: NetPolicy,
}

impl Fetcher {
    /// `loopback` also allows 127.0.0.1, for tests against a local server. The script's cookies
    /// live in `cookies` (M46.11).
    pub fn new(loopback: bool, cookies: Arc<CookieJar>) -> Result<Self, String> {
        let mut patterns = vec![HostPattern::AnyPublic];
        if loopback {
            patterns.push(HostPattern::Exact("127.0.0.1".into()));
        }
        let policy = NetPolicy::new(patterns);
        let redirect_policy = policy.clone();
        let redirects = reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS {
                attempt.error("too many redirects")
            } else if redirect_policy.allows(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("redirect to a local address")
            }
        });
        let client = raygun_net::client_builder()
            .dns_resolver(Arc::new(PublicResolver))
            .redirect(redirects)
            .cookie_provider(cookies)
            .timeout(TIMEOUT)
            .build()
            .map_err(|error| error.to_string())?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            client,
            runtime,
            policy,
        })
    }

    pub fn execute(&self, request: &LuaRequest) -> Result<LuaResponse, String> {
        let url = reqwest::Url::parse(&request.url)
            .map_err(|error| format!("invalid URL {:?}: {error}", request.url))?;
        if !self.policy.allows(&url) {
            return Err(format!("{url} is not a public web address"));
        }
        let method = reqwest::Method::from_bytes(request.method.to_ascii_uppercase().as_bytes())
            .map_err(|_| format!("unknown method {}", request.method))?;
        let mut builder = self.client.request(method, url.clone());
        let mut has_agent = false;
        for (name, value) in &request.headers {
            has_agent |= name.eq_ignore_ascii_case("user-agent");
            builder = builder.header(name, value);
        }
        if !has_agent {
            builder = builder.header("User-Agent", raygun_net::user_agent());
        }
        if let Some(body) = &request.body {
            builder = builder
                .header("Content-Type", &body.content_type)
                .body(body.bytes.clone());
        }
        self.runtime.block_on(async {
            let response = builder.send().await.map_err(|error| describe(&error))?;
            let status = response.status().as_u16();
            let final_url = response.url().to_string();
            let headers = response
                .headers()
                .iter()
                .filter_map(|(name, value)| {
                    Some((name.to_string(), value.to_str().ok()?.to_owned()))
                })
                .collect();
            if response
                .content_length()
                .is_some_and(|length| length > MAX_BODY_BYTES as u64)
            {
                return Err(format!("{final_url} sent more than 16 MiB"));
            }
            let mut body = Vec::new();
            let mut response = response;
            while let Some(chunk) = response.chunk().await.map_err(|error| describe(&error))? {
                if body.len() + chunk.len() > MAX_BODY_BYTES {
                    return Err(format!("{final_url} sent more than 16 MiB"));
                }
                body.extend_from_slice(&chunk);
            }
            Ok(LuaResponse {
                status,
                url: final_url,
                headers,
                body,
            })
        })
    }
}

/// The innermost cause ("operation timed out", a refused address) is what a user needs.
fn describe(error: &reqwest::Error) -> String {
    let mut message = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        message = cause.to_string();
        source = cause.source();
    }
    message
}

#[derive(Clone, Default)]
pub struct LuaHeaders(pub Vec<(String, String)>);

#[derive(Default)]
struct HeadersBuilder(Vec<(String, String)>);

#[derive(Clone)]
pub struct LuaBody {
    content_type: String,
    bytes: Vec<u8>,
}

#[derive(Default)]
struct FormBuilder(Vec<(String, String)>);

#[derive(Clone)]
pub struct LuaRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<LuaBody>,
}

struct RequestBuilder(LuaRequest);

#[derive(Clone)]
pub struct LuaResponse {
    status: u16,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

struct ResponseBody(Vec<u8>);

fn header(headers: &[(String, String)], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.clone())
}

fn form_encode(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

impl UserData for LuaHeaders {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get", |_, this, name: String| Ok(header(&this.0, &name)));
        methods.add_method("size", |_, this, ()| Ok(this.0.len()));
        methods.add_method("names", |lua, this, ()| {
            lua.create_sequence_from(this.0.iter().map(|(name, _)| name.clone()))
        });
        methods.add_method("newBuilder", |_, this, ()| {
            Ok(HeadersBuilder(this.0.clone()))
        });
    }
}

impl UserData for HeadersBuilder {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_function(
            "add",
            |_, (this, name, value): (AnyUserData, String, String)| {
                this.borrow_mut::<HeadersBuilder>()?.0.push((name, value));
                Ok(this)
            },
        );
        methods.add_function(
            "set",
            |_, (this, name, value): (AnyUserData, String, String)| {
                {
                    let mut builder = this.borrow_mut::<HeadersBuilder>()?;
                    builder
                        .0
                        .retain(|(key, _)| !key.eq_ignore_ascii_case(&name));
                    builder.0.push((name, value));
                }
                Ok(this)
            },
        );
        methods.add_function("removeAll", |_, (this, name): (AnyUserData, String)| {
            this.borrow_mut::<HeadersBuilder>()?
                .0
                .retain(|(key, _)| !key.eq_ignore_ascii_case(&name));
            Ok(this)
        });
        methods.add_method("build", |_, this, ()| Ok(LuaHeaders(this.0.clone())));
    }
}

impl UserData for LuaBody {}

impl UserData for FormBuilder {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_function(
            "add",
            |_, (this, name, value): (AnyUserData, String, String)| {
                this.borrow_mut::<FormBuilder>()?
                    .0
                    .push((form_encode(&name), form_encode(&value)));
                Ok(this)
            },
        );
        methods.add_function(
            "addEncoded",
            |_, (this, name, value): (AnyUserData, String, String)| {
                this.borrow_mut::<FormBuilder>()?.0.push((name, value));
                Ok(this)
            },
        );
        methods.add_method("build", |_, this, ()| {
            let text = this
                .0
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join("&");
            Ok(LuaBody {
                content_type: "application/x-www-form-urlencoded".into(),
                bytes: text.into_bytes(),
            })
        });
    }
}

impl UserData for LuaRequest {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("url", |_, this, ()| Ok(this.url.clone()));
        methods.add_method("method", |_, this, ()| Ok(this.method.clone()));
        methods.add_method("header", |_, this, name: String| {
            Ok(header(&this.headers, &name))
        });
        methods.add_method("headers", |_, this, ()| {
            Ok(LuaHeaders(this.headers.clone()))
        });
        methods.add_method("newBuilder", |_, this, ()| Ok(RequestBuilder(this.clone())));
    }
}

impl UserData for RequestBuilder {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_function("url", |_, (this, url): (AnyUserData, String)| {
            this.borrow_mut::<RequestBuilder>()?.0.url = url;
            Ok(this)
        });
        for name in ["header", "addHeader"] {
            let replace = name == "header";
            methods.add_function(
                name,
                move |_, (this, key, value): (AnyUserData, String, String)| {
                    {
                        let mut builder = this.borrow_mut::<RequestBuilder>()?;
                        if replace {
                            builder
                                .0
                                .headers
                                .retain(|(existing, _)| !existing.eq_ignore_ascii_case(&key));
                        }
                        builder.0.headers.push((key, value));
                    }
                    Ok(this)
                },
            );
        }
        methods.add_function("removeHeader", |_, (this, key): (AnyUserData, String)| {
            this.borrow_mut::<RequestBuilder>()?
                .0
                .headers
                .retain(|(existing, _)| !existing.eq_ignore_ascii_case(&key));
            Ok(this)
        });
        methods.add_function(
            "headers",
            |_, (this, headers): (AnyUserData, AnyUserData)| {
                let headers = headers.borrow::<LuaHeaders>()?.0.clone();
                this.borrow_mut::<RequestBuilder>()?.0.headers = headers;
                Ok(this)
            },
        );
        methods.add_function("get", |_, this: AnyUserData| {
            {
                let mut builder = this.borrow_mut::<RequestBuilder>()?;
                builder.0.method = "GET".into();
                builder.0.body = None;
            }
            Ok(this)
        });
        methods.add_function("post", |_, (this, body): (AnyUserData, AnyUserData)| {
            let body = body.borrow::<LuaBody>()?.clone();
            {
                let mut builder = this.borrow_mut::<RequestBuilder>()?;
                builder.0.method = "POST".into();
                builder.0.body = Some(body);
            }
            Ok(this)
        });
        methods.add_function(
            "method",
            |_, (this, method, body): (AnyUserData, String, Option<AnyUserData>)| {
                let body = body
                    .map(|body| body.borrow::<LuaBody>().map(|body| body.clone()))
                    .transpose()?;
                {
                    let mut builder = this.borrow_mut::<RequestBuilder>()?;
                    builder.0.method = method;
                    builder.0.body = body;
                }
                Ok(this)
            },
        );
        methods.add_function("cacheControl", |_, (this, _): (AnyUserData, Value)| {
            Ok(this)
        });
        methods.add_method("build", |_, this, ()| Ok(this.0.clone()));
    }
}

impl UserData for LuaResponse {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("code", |_, this, ()| Ok(this.status));
        methods.add_method("isSuccessful", |_, this, ()| {
            Ok((200..300).contains(&this.status))
        });
        methods.add_method("message", |_, this, ()| Ok(this.status.to_string()));
        methods.add_method("body", |_, this, ()| Ok(ResponseBody(this.body.clone())));
        methods.add_method("headers", |_, this, ()| {
            Ok(LuaHeaders(this.headers.clone()))
        });
        methods.add_method("header", |_, this, name: String| {
            Ok(header(&this.headers, &name))
        });
        methods.add_method("request", |lua, this, ()| {
            let request = lua.create_table()?;
            request.set("url", this.url.clone())?;
            Ok(request)
        });
        methods.add_method("close", |_, _, ()| Ok(()));
    }
}

impl UserData for ResponseBody {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("string", |_, this, ()| {
            Ok(String::from_utf8_lossy(&this.0).into_owned())
        });
        methods.add_method("close", |_, _, ()| Ok(()));
    }
}

fn request(
    method: &str,
    url: String,
    headers: Option<AnyUserData>,
    body: Option<LuaBody>,
) -> mlua::Result<LuaRequest> {
    let headers = match headers {
        Some(headers) => headers.borrow::<LuaHeaders>()?.0.clone(),
        None => Vec::new(),
    };
    Ok(LuaRequest {
        method: method.into(),
        url,
        headers,
        body,
    })
}

/// A document from a successful response, or the HTTP status as the error.
fn document(fetcher: &Fetcher, request: &LuaRequest) -> mlua::Result<LuaNode> {
    let response = fetcher.execute(request).map_err(mlua::Error::runtime)?;
    if !(200..300).contains(&response.status) {
        return Err(mlua::Error::runtime(format!(
            "HTTP {} from {}",
            response.status, response.url
        )));
    }
    let base = reqwest::Url::parse(&response.url).ok();
    Ok(parse_document(
        &String::from_utf8_lossy(&response.body),
        base,
    ))
}

pub fn install(lua: &Lua, globals: &Table, fetcher: Arc<Fetcher>) -> mlua::Result<()> {
    globals.set(
        "DEFAULT_HEADERS",
        lua.create_function(|_, ()| Ok(LuaHeaders::default()))?,
    )?;
    globals.set(
        "DEFAULT_CACHE_CONTROL",
        lua.create_function(|_, ()| Ok(true))?,
    )?;
    globals.set(
        "DEFAULT_BODY",
        lua.create_function(|_, ()| {
            Ok(LuaBody {
                content_type: "application/x-www-form-urlencoded".into(),
                bytes: Vec::new(),
            })
        })?,
    )?;
    globals.set(
        "_GET",
        lua.create_function(
            |_, (url, headers, _cache): (String, Option<AnyUserData>, Value)| {
                request("GET", url, headers, None)
            },
        )?,
    )?;
    globals.set(
        "_POST",
        lua.create_function(
            |_,
             (url, headers, body, _cache): (
                String,
                Option<AnyUserData>,
                Option<AnyUserData>,
                Value,
            )| {
                let body = body
                    .map(|body| body.borrow::<LuaBody>().map(|body| body.clone()))
                    .transpose()?;
                request("POST", url, headers, body)
            },
        )?,
    )?;
    let run = fetcher.clone();
    globals.set(
        "Request",
        lua.create_function(move |_, request: AnyUserData| {
            run.execute(&*request.borrow::<LuaRequest>()?)
                .map_err(mlua::Error::runtime)
        })?,
    )?;
    let run = fetcher.clone();
    globals.set(
        "RequestDocument",
        lua.create_function(move |_, request: AnyUserData| {
            document(&run, &*request.borrow::<LuaRequest>()?)
        })?,
    )?;
    let run = fetcher;
    globals.set(
        "GETDocument",
        lua.create_function(move |_, url: String| {
            document(&run, &request("GET", url, None, None)?)
        })?,
    )?;
    globals.set(
        "HeadersBuilder",
        lua.create_function(|_, ()| Ok(HeadersBuilder::default()))?,
    )?;
    globals.set(
        "FormBodyBuilder",
        lua.create_function(|_, ()| Ok(FormBuilder::default()))?,
    )?;
    globals.set(
        "RequestBuilder",
        lua.create_function(|_, ()| {
            Ok(RequestBuilder(LuaRequest {
                method: "GET".into(),
                url: String::new(),
                headers: Vec::new(),
                body: None,
            }))
        })?,
    )?;
    globals.set(
        "MediaType",
        lua.create_function(|_, kind: String| Ok(kind))?,
    )?;
    globals.set(
        "RequestBody",
        lua.create_function(|_, (data, kind): (mlua::String, Option<String>)| {
            Ok(LuaBody {
                content_type: kind.unwrap_or_else(|| "text/plain; charset=utf-8".into()),
                bytes: data.as_bytes().to_vec(),
            })
        })?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_local_addresses() {
        let fetcher = Fetcher::new(false, CookieJar::in_memory()).unwrap();
        for url in [
            "http://127.0.0.1/",
            "http://localhost:8080/",
            "http://192.168.1.10/",
            "http://10.0.2.2/",
            "http://[::1]/",
            "http://router.lan/",
            "file:///etc/passwd",
        ] {
            let request = request("GET", url.into(), None, None).unwrap();
            let error = fetcher.execute(&request).err().unwrap_or_default();
            assert!(
                error.contains("not a public") || error.contains("invalid URL"),
                "{url}: {error}"
            );
        }
    }
}
