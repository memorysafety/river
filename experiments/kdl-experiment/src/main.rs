#![allow(dead_code)]

use std::{
    collections::BTreeMap,
    fs::read_to_string,
    net::SocketAddr,
    path::PathBuf,
};

use config::{Config, ListenerConfig, ListenerKind, PathControl, ProxyConfig, TlsConfig};
use kdl::{KdlDocument, KdlEntry, KdlNode};
use miette::{Diagnostic, SourceSpan};
use pingora::upstreams::peer::HttpPeer;

mod config;

fn main() {
    inner_main().unwrap();
}

fn inner_main() -> miette::Result<()> {
    let kdl_contents = read_to_string("./reference.kdl").unwrap();
    // println!("KDL\n{kdl_contents:?}");
    let doc: KdlDocument = kdl_contents.parse()?;

    let val: Config = doc.try_into()?;

    println!("{val:#?}");

    Ok(())
}

impl TryFrom<KdlDocument> for Config {
    type Error = miette::Error;

    fn try_from(value: KdlDocument) -> Result<Self, Self::Error> {
        let threads_per_service = extract_threads_per_service(&value)?;
        let basic_proxies = extract_services(&value)?;

        Ok(Config {
            threads_per_service,
            basic_proxies,
            ..Config::default()
        })
    }
}

fn required_child_doc<'a>(
    doc: &KdlDocument,
    here: &'a KdlDocument,
    name: &str,
) -> miette::Result<&'a KdlDocument> {
    let node = here
        .get(name)
        .or_bail(&format!("'{name}' is required!"), doc, here.span())?;

    node.children()
        .or_bail("expected a nested node", doc, node.span())
}

fn optional_child_doc<'a>(
    _doc: &KdlDocument,
    here: &'a KdlDocument,
    name: &str,
) -> Option<&'a KdlDocument> {
    let node = here.get(name)?;

    node.children()
}

fn wildcard_argless_child_docs<'a>(
    doc: &KdlDocument,
    here: &'a KdlDocument,
) -> miette::Result<Vec<(&'a str, &'a KdlDocument)>> {
    // TODO: assert no args?
    let mut children = vec![];
    for node in here.nodes() {
        let name = node.name().value();
        let child = node.children().or_bail(
            &format!("'{name}' should be a nested block"),
            doc,
            node.span(),
        )?;
        children.push((name, child));
    }
    Ok(children)
}

/// Intended to be used with the internal nodes of a section, for example:
///
/// ```kdl
/// listeners {
/// //  vvvvvvvvvvvvvv <-------------------------------------- These are the &'str name parts
///     "0.0.0.0:8080"                               // <\
///     "0.0.0.0:4443"                               // <----- These are the data nodes
///     "0.0.0.0:8443" cert-path="./assets/test.crt" // </
/// //                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ <-------- These are the KdlEntry parts
/// }
/// ```
fn data_nodes<'a>(
    _doc: &KdlDocument,
    here: &'a KdlDocument,
) -> miette::Result<Vec<(&'a KdlNode, &'a str, &'a [KdlEntry])>> {
    let mut out = vec![];
    for node in here.nodes() {
        out.push((node, node.name().value(), node.entries()));
    }
    Ok(out)
}

/// Extract all services from the top level document
fn extract_services(doc: &KdlDocument) -> miette::Result<Vec<ProxyConfig>> {
    let service_node = required_child_doc(doc, doc, "services")?;
    let services = wildcard_argless_child_docs(doc, service_node)?;

    let mut proxies = vec![];
    for (name, service) in services {
        proxies.push(extract_service(doc, name, service)?);
    }
    Ok(proxies)
}

/// Collects all the filters, where the node name must be "filter", and the rest of the args
/// are collected as a BTreeMap of String:String values
///
/// ```kdl
/// upstream-request {
///     filter kind="remove-header-key-regex" pattern=".*SECRET.*"
///     filter kind="remove-header-key-regex" pattern=".*secret.*"
///     filter kind="upsert-header" key="x-proxy-friend" value="river"
/// }
/// ```
///
/// creates something like:
///
/// ```json
/// [
///     { kind: "remove-header-key-regex", pattern: ".*SECRET.*" },
///     { kind: "remove-header-key-regex", pattern: ".*secret.*" },
///     { kind: "upsert-header", key: "x-proxy-friend", value: "river" }
/// ]
/// ```
fn collect_filters(
    doc: &KdlDocument,
    node: &KdlDocument,
) -> miette::Result<Vec<BTreeMap<String, String>>> {
    let filters = data_nodes(doc, node)?;
    let mut fout = vec![];
    for (_node, name, args) in filters {
        if name != "filter" {
            panic!()
        }
        let args = str_str_args(doc, args)?;
        fout.push(
            args.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        );
    }
    Ok(fout)
}

/// Extracts a single service from the `services` block
fn extract_service(
    doc: &KdlDocument,
    name: &str,
    node: &KdlDocument,
) -> miette::Result<ProxyConfig> {
    // Listeners
    //
    let listener_node = required_child_doc(doc, node, "listeners")?;
    let listeners = data_nodes(doc, listener_node)?;
    if listeners.is_empty() {
        return Err(Bad::docspan("nonzero listeners required", doc, listener_node.span()).into());
    }
    let mut list_cfgs = vec![];
    for (node, name, args) in listeners {
        let listener = extract_listener(doc, node, name, args)?;
        list_cfgs.push(listener);
    }

    // Connectors
    //
    let conn_node = required_child_doc(doc, node, "connectors")?;
    let conns = data_nodes(doc, conn_node)?;
    if conns.len() != 1 {
        return Err(Bad::docspan("exactly one connector required", doc, conn_node.span()).into());
    }
    let mut conn_cfgs = vec![];
    for (node, name, args) in conns {
        let conn = extract_connector(doc, node, name, args)?;
        conn_cfgs.push(conn);
    }

    // Path Control (optional)
    //
    let mut pc = PathControl::default();
    if let Some(pc_node) = optional_child_doc(doc, node, "path-control") {
        // upstream-request (optional)
        if let Some(ureq_node) = optional_child_doc(doc, pc_node, "upstream-request") {
            pc.upstream_request_filters = collect_filters(doc, ureq_node)?;
        }

        // upstream-response (optional)
        if let Some(uresp_node) = optional_child_doc(doc, pc_node, "upstream-response") {
            pc.upstream_response_filters = collect_filters(doc, uresp_node)?
        }
    }

    Ok(ProxyConfig {
        name: name.to_string(),
        listeners: list_cfgs,
        upstream: conn_cfgs.pop().unwrap(),
        path_control: pc,
    })
}

/// Useful for collecting all arguments as str:str key pairs
fn str_str_args<'a>(
    doc: &KdlDocument,
    args: &'a [KdlEntry],
) -> miette::Result<Vec<(&'a str, &'a str)>> {
    let mut out = vec![];
    for arg in args {
        let name =
            arg.name()
                .map(|a| a.value())
                .or_bail("arguments should be named", doc, arg.span())?;
        let val =
            arg.value()
                .as_string()
                .or_bail("arg values should be a string", doc, arg.span())?;
        out.push((name, val));
    }
    Ok(out)
}

/// Extracts a single connector from the `connectors` section
fn extract_connector(
    doc: &KdlDocument,
    node: &KdlNode,
    name: &str,
    args: &[KdlEntry],
) -> miette::Result<HttpPeer> {
    let Ok(sadd) = name.parse::<SocketAddr>() else {
        return Err(Bad::docspan("Not a valid socket address", doc, node.span()).into());
    };

    let args = str_str_args(doc, args)?;
    let (tls, sni) = match args.as_slice() {
        [] => (false, String::new()),
        [("tls-sni", sni)] => (true, sni.to_string()),
        _ => {
            return Err(Bad::docspan(
                "This should have zero args or just 'tls-sni'",
                doc,
                node.span(),
            )
            .into());
        }
    };

    Ok(HttpPeer::new(sadd, tls, sni))
}

// services { Service { listeners { ... } } }
fn extract_listener(
    doc: &KdlDocument,
    node: &KdlNode,
    name: &str,
    args: &[KdlEntry],
) -> miette::Result<ListenerConfig> {
    let mut args = str_str_args(doc, args)?;
    args.sort_by_key(|a| a.0);

    // Is this a bindable name?
    if name.parse::<SocketAddr>().is_ok() {
        // Cool: do we have reasonable args for this?
        match args.as_slice() {
            // No argument - it's a regular TCP listener
            [] => Ok(ListenerConfig {
                source: ListenerKind::Tcp {
                    addr: name.to_string(),
                    tls: None,
                },
            }),
            // exactly these two args: it's a TLS listener
            [("cert-path", cpath), ("key-path", kpath)] => Ok(ListenerConfig {
                source: ListenerKind::Tcp {
                    addr: name.to_string(),
                    tls: Some(TlsConfig {
                        cert_path: cpath.into(),
                        key_path: kpath.into(),
                    }),
                },
            }),
            // Otherwise, I dunno what this is
            _ => {
                return Err(Bad::docspan(
                    "listeners must have no args or both cert-path and key-path",
                    doc,
                    node.span(),
                )
                .into())
            }
        }
    } else if let Ok(pb) = name.parse::<PathBuf>() {
        // TODO: Should we check that this path exists? Otherwise it seems to always match
        Ok(ListenerConfig {
            source: ListenerKind::Uds(pb),
        })
    } else {
        Err(Bad::docspan("'{name}' is not a socketaddr or path?", doc, node.span()).into())
    }
}

// system { threads-per-service N }
fn extract_threads_per_service(doc: &KdlDocument) -> miette::Result<usize> {
    let Some(tps) =
        optional_child_doc(doc, doc, "system").and_then(|n| n.get("threads-per-service"))
    else {
        // Not present, go ahead and return the default
        return Ok(8);
    };

    let [tps_node] = tps.entries() else {
        return Err(Bad::docspan(
            "system > threads-per-service should have exactly one entry",
            doc,
            tps.span(),
        )
        .into());
    };

    let val = tps_node.value().as_i64().or_bail(
        "system > threads-per-service should be an integer",
        doc,
        tps_node.span(),
    )?;
    val.try_into().ok().or_bail(
        "system > threads-per-service should fit in a usize",
        doc,
        tps_node.span(),
    )
}

#[derive(thiserror::Error, Debug, Diagnostic)]
#[error("Incorrect configuration contents")]
struct Bad {
    #[help]
    error: String,

    #[source_code]
    src: String,

    #[label("incorrect")]
    err_span: SourceSpan,
}

trait OptExtParse {
    type Good;

    fn or_bail(
        self,
        msg: impl Into<String>,
        doc: &KdlDocument,
        span: &SourceSpan,
    ) -> miette::Result<Self::Good>;
}

impl<T> OptExtParse for Option<T> {
    type Good = T;

    fn or_bail(
        self,
        msg: impl Into<String>,
        doc: &KdlDocument,
        span: &SourceSpan,
    ) -> miette::Result<Self::Good> {
        match self {
            Some(t) => Ok(t),
            None => Err(Bad::docspan(msg, doc, span).into()),
        }
    }
}

impl Bad {
    fn docspan(msg: impl Into<String>, doc: &KdlDocument, span: &SourceSpan) -> Self {
        Self {
            error: msg.into(),
            src: doc.to_string(),
            err_span: span.to_owned(),
        }
    }
}
