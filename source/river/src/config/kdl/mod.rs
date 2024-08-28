use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::SocketAddr,
    path::PathBuf,
};

use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use miette::{bail, Diagnostic, SourceSpan};
use pingora::{protocols::ALPN, upstreams::peer::HttpPeer};
use regex::Regex;

use crate::{
    config::internal::{
        Config, DiscoveryKind, FileServerConfig, HealthCheckKind, ListenerConfig, ListenerKind,
        PathControl, ProxyConfig, SelectionKind, TlsConfig, UpstreamOptions,
    },
    proxy::{
        rate_limiting::{RaterConfig, RaterInstanceConfig, RequestKeyKind},
        request_selector::{
            null_selector, source_addr_and_uri_path_selector, uri_path_selector, RequestSelector,
        },
    },
};

use super::internal::RateLimitingConfig;

#[cfg(test)]
mod test;
mod utils;

/// This is the primary interface for parsing the document.
impl TryFrom<KdlDocument> for Config {
    type Error = miette::Error;

    fn try_from(value: KdlDocument) -> Result<Self, Self::Error> {
        let SystemData {
            threads_per_service,
            daemonize,
            upgrade_socket,
            pid_file,
        } = extract_system_data(&value)?;
        let (basic_proxies, file_servers) = extract_services(threads_per_service, &value)?;

        Ok(Config {
            threads_per_service,
            daemonize,
            upgrade_socket,
            pid_file,
            basic_proxies,
            file_servers,
            ..Config::default()
        })
    }
}

struct SystemData {
    threads_per_service: usize,
    daemonize: bool,
    upgrade_socket: Option<PathBuf>,
    pid_file: Option<PathBuf>,
}

impl Default for SystemData {
    fn default() -> Self {
        Self {
            threads_per_service: 8,
            daemonize: false,
            upgrade_socket: None,
            pid_file: None,
        }
    }
}

/// Extract all services from the top level document
fn extract_services(
    threads_per_service: usize,
    doc: &KdlDocument,
) -> miette::Result<(Vec<ProxyConfig>, Vec<FileServerConfig>)> {
    let service_node = utils::required_child_doc(doc, doc, "services")?;
    let services = utils::wildcard_argless_child_docs(doc, service_node)?;

    let proxy_node_set =
        HashSet::from(["listeners", "connectors", "path-control", "rate-limiting"]);
    let file_server_node_set = HashSet::from(["listeners", "file-server"]);

    let mut proxies = vec![];
    let mut file_servers = vec![];

    for (name, service) in services {
        // First, visit all of the children nodes, and make sure each child
        // node only appears once. This is used to detect duplicate sections
        let mut fingerprint_set: HashSet<&str> = HashSet::new();
        for ch in service.nodes() {
            let name = ch.name().value();
            let dupe = !fingerprint_set.insert(name);
            if dupe {
                return Err(
                    Bad::docspan(format!("Duplicate section: '{name}'!"), doc, ch.span()).into(),
                );
            }
        }

        // Now: what do we do with this node?
        if fingerprint_set.is_subset(&proxy_node_set) {
            // If the contained nodes are a strict subset of proxy node config fields,
            // then treat this section as a proxy node
            proxies.push(extract_service(threads_per_service, doc, name, service)?);
        } else if fingerprint_set.is_subset(&file_server_node_set) {
            // If the contained nodes are a strict subset of the file server config
            // fields, then treat this section as a file server node
            file_servers.push(extract_file_server(doc, name, service)?);
        } else {
            // Otherwise, we're not sure what this node is supposed to be!
            //
            // Obtain the superset of ALL potential nodes, which is essentially
            // our configuration grammar.
            let superset: HashSet<&str> = proxy_node_set
                .union(&file_server_node_set)
                .cloned()
                .collect();

            // Then figure out what fields our fingerprint set contains that
            // is "novel", or basically fields we don't know about
            let what = fingerprint_set
                .difference(&superset)
                .copied()
                .collect::<Vec<&str>>()
                .join(", ");

            // Then inform the user about the reason for our discontent
            return Err(Bad::docspan(
                format!("Unknown configuration section(s): {what}"),
                doc,
                service.span(),
            )
            .into());
        }
    }

    if proxies.is_empty() && file_servers.is_empty() {
        return Err(Bad::docspan("No services defined", doc, service_node.span()).into());
    }

    Ok((proxies, file_servers))
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
    let filters = utils::data_nodes(doc, node)?;
    let mut fout = vec![];
    for (_node, name, args) in filters {
        if name != "filter" {
            bail!("Invalid Filter Rule");
        }
        let args = utils::str_str_args(doc, args)?;
        fout.push(
            args.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        );
    }
    Ok(fout)
}

/// Extracts a single file server from the `services` block
fn extract_file_server(
    doc: &KdlDocument,
    name: &str,
    node: &KdlDocument,
) -> miette::Result<FileServerConfig> {
    // Listeners
    //
    let listener_node = utils::required_child_doc(doc, node, "listeners")?;
    let listeners = utils::data_nodes(doc, listener_node)?;
    if listeners.is_empty() {
        return Err(Bad::docspan("nonzero listeners required", doc, listener_node.span()).into());
    }
    let mut list_cfgs = vec![];
    for (node, name, args) in listeners {
        let listener = extract_listener(doc, node, name, args)?;
        list_cfgs.push(listener);
    }

    // Base Path
    //
    let fs_node = utils::required_child_doc(doc, node, "file-server")?;
    let data_nodes = utils::data_nodes(doc, fs_node)?;
    let mut map = HashMap::new();
    for (node, name, args) in data_nodes {
        map.insert(name, (node, args));
    }

    let base_path = if let Some((bpnode, bpargs)) = map.get("base-path") {
        let val =
            utils::extract_one_str_arg(doc, bpnode, "base-path", bpargs, |a| Some(a.to_string()))?;
        Some(val.into())
    } else {
        None
    };

    Ok(FileServerConfig {
        name: name.to_string(),
        listeners: list_cfgs,
        base_path,
    })
}

/// Extracts a single service from the `services` block
fn extract_service(
    threads_per_service: usize,
    doc: &KdlDocument,
    name: &str,
    node: &KdlDocument,
) -> miette::Result<ProxyConfig> {
    // Listeners
    //
    let listener_node = utils::required_child_doc(doc, node, "listeners")?;
    let listeners = utils::data_nodes(doc, listener_node)?;
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
    let conn_node = utils::required_child_doc(doc, node, "connectors")?;
    let conns = utils::data_nodes(doc, conn_node)?;
    let mut conn_cfgs = vec![];
    let mut load_balance: Option<UpstreamOptions> = None;
    for (node, name, args) in conns {
        if name == "load-balance" {
            if load_balance.is_some() {
                panic!("Don't have two 'load-balance' sections");
            }
            load_balance = Some(extract_load_balance(doc, node)?);
            continue;
        }
        let conn = extract_connector(doc, node, name, args)?;
        conn_cfgs.push(conn);
    }
    if conn_cfgs.is_empty() {
        return Err(
            Bad::docspan("We require at least one connector", doc, conn_node.span()).into(),
        );
    }

    // Path Control (optional)
    //
    let mut pc = PathControl::default();
    if let Some(pc_node) = utils::optional_child_doc(doc, node, "path-control") {
        // request-filters (optional)
        if let Some(ureq_node) = utils::optional_child_doc(doc, pc_node, "request-filters") {
            pc.request_filters = collect_filters(doc, ureq_node)?;
        }

        // upstream-request (optional)
        if let Some(ureq_node) = utils::optional_child_doc(doc, pc_node, "upstream-request") {
            pc.upstream_request_filters = collect_filters(doc, ureq_node)?;
        }

        // upstream-response (optional)
        if let Some(uresp_node) = utils::optional_child_doc(doc, pc_node, "upstream-response") {
            pc.upstream_response_filters = collect_filters(doc, uresp_node)?
        }
    }

    // Rate limiting
    let mut rl = RateLimitingConfig::default();
    if let Some(rl_node) = utils::optional_child_doc(doc, node, "rate-limiting") {
        let nodes = utils::data_nodes(doc, rl_node)?;
        for (node, name, args) in nodes.iter() {
            match *name {
                "timeout" => {
                    let vals = utils::str_value_args(doc, args)?;
                    let [("millis", val)] = vals.as_slice() else {
                        return Err(Bad::docspan(
                            "Unknown argument, missing 'millis'",
                            doc,
                            node.span(),
                        )
                        .into());
                    };
                    let Some(millis) = val.value().as_i64().and_then(|v| usize::try_from(v).ok())
                    else {
                        return Err(Bad::docspan(
                            "'millis' value should be a number",
                            doc,
                            node.span(),
                        )
                        .into());
                    };
                    let old = rl.timeout_ms.replace(millis);
                    if old.is_some() {
                        return Err(Bad::docspan(
                            "Can't specify 'timeout' twice!",
                            doc,
                            node.span(),
                        )
                        .into());
                    }
                }
                "rule" => {
                    let vals = utils::str_value_args(doc, args)?;
                    let valslice = vals
                        .iter()
                        .map(|(k, v)| (*k, v.value()))
                        .collect::<BTreeMap<&str, &KdlValue>>();
                    rl.rules
                        .push(make_rate_limiter(threads_per_service, doc, node, valslice)?);
                }
                other => {
                    return Err(
                        Bad::docspan(format!("Unknown name: '{other}'"), doc, node.span()).into(),
                    );
                }
            }
        }
    }

    Ok(ProxyConfig {
        name: name.to_string(),
        listeners: list_cfgs,
        upstreams: conn_cfgs,
        path_control: pc,
        upstream_options: load_balance.unwrap_or_default(),
        rate_limiting: rl,
    })
}

fn make_rate_limiter(
    threads_per_service: usize,
    doc: &KdlDocument,
    node: &KdlNode,
    args: BTreeMap<&str, &KdlValue>,
) -> miette::Result<RaterInstanceConfig> {
    let take_num = |key: &str| -> miette::Result<usize> {
        let Some(val) = args.get(key) else {
            return Err(Bad::docspan(format!("Missing key: '{key}'"), doc, node.span()).into());
        };
        let Some(val) = val.as_i64().and_then(|v| usize::try_from(v).ok()) else {
            return Err(Bad::docspan(
                format!(
                    "'{key} should have a positive integer value, got '{:?}' instead",
                    val
                ),
                doc,
                node.span(),
            )
            .into());
        };
        Ok(val)
    };
    let take_str = |key: &str| -> miette::Result<&str> {
        let Some(val) = args.get(key) else {
            return Err(Bad::docspan(format!("Missing key: '{key}'"), doc, node.span()).into());
        };
        let Some(val) = val.as_string() else {
            return Err(Bad::docspan(
                format!("'{key} should have a string value, got '{:?}' instead", val),
                doc,
                node.span(),
            )
            .into());
        };
        Ok(val)
    };

    // mandatory/common fields
    let kind = take_str("kind")?;
    let max_buckets = take_num("max-buckets")?;
    let tokens_per_bucket = take_num("tokens-per-bucket")?;
    let refill_qty = take_num("refill-qty")?;
    let refill_rate_ms = take_num("refill-rate-ms")?;

    let rater_cfg = RaterConfig {
        threads: threads_per_service,
        max_buckets,
        max_tokens_per_bucket: tokens_per_bucket,
        refill_interval_millis: refill_rate_ms,
        refill_qty,
    };

    match kind {
        "source-ip" => Ok(RaterInstanceConfig {
            rater_cfg,
            kind: RequestKeyKind::SourceIp,
        }),
        "uri" => {
            let pattern = take_str("pattern")?;
            let Ok(pattern) = Regex::new(pattern) else {
                return Err(Bad::docspan(
                    format!("'{pattern} should be a valid regular expression"),
                    doc,
                    node.span(),
                )
                .into());
            };
            Ok(RaterInstanceConfig {
                rater_cfg,
                kind: RequestKeyKind::Uri { pattern },
            })
        }
        other => Err(Bad::docspan(
            format!("'{other} is not a known kind of rate limiting"),
            doc,
            node.span(),
        )
        .into()),
    }
}

/// Extracts the `load-balance` structure from the `connectors` section
fn extract_load_balance(doc: &KdlDocument, node: &KdlNode) -> miette::Result<UpstreamOptions> {
    let items = utils::data_nodes(
        doc,
        node.children()
            .or_bail("'load-balance' should have children", doc, node.span())?,
    )?;

    let mut selection: Option<SelectionKind> = None;
    let mut health: Option<HealthCheckKind> = None;
    let mut discover: Option<DiscoveryKind> = None;
    let mut selector: RequestSelector = null_selector;

    for (node, name, args) in items {
        match name {
            "selection" => {
                let (sel, args) = utils::extract_one_str_arg_with_kv_args(
                    doc,
                    node,
                    name,
                    args,
                    |val| match val {
                        "RoundRobin" => Some(SelectionKind::RoundRobin),
                        "Random" => Some(SelectionKind::Random),
                        "FNV" => Some(SelectionKind::Fnv),
                        "Ketama" => Some(SelectionKind::Ketama),
                        _ => None,
                    },
                )?;
                match sel {
                    SelectionKind::RoundRobin | SelectionKind::Random => {
                        // No key required, selection is random
                    }
                    SelectionKind::Fnv | SelectionKind::Ketama => {
                        let sel_ty = args.get("key").or_bail(
                            format!("selection {sel:?} requires a 'key' argument"),
                            doc,
                            node.span(),
                        )?;

                        selector = match sel_ty.as_str() {
                            "UriPath" => uri_path_selector,
                            "SourceAddrAndUriPath" => source_addr_and_uri_path_selector,
                            other => {
                                return Err(Bad::docspan(
                                    format!("Unknown key: '{other}'"),
                                    doc,
                                    node.span(),
                                )
                                .into())
                            }
                        };
                    }
                }

                selection = Some(sel);
            }
            "health-check" => {
                health = Some(utils::extract_one_str_arg(
                    doc,
                    node,
                    name,
                    args,
                    |val| match val {
                        "None" => Some(HealthCheckKind::None),
                        _ => None,
                    },
                )?);
            }
            "discovery" => {
                discover = Some(utils::extract_one_str_arg(
                    doc,
                    node,
                    name,
                    args,
                    |val| match val {
                        "Static" => Some(DiscoveryKind::Static),
                        _ => None,
                    },
                )?);
            }
            other => {
                return Err(
                    Bad::docspan(format!("Unknown setting: '{other}'"), doc, node.span()).into(),
                );
            }
        }
    }
    Ok(UpstreamOptions {
        selection: selection.unwrap_or(SelectionKind::RoundRobin),
        selector,
        health_checks: health.unwrap_or(HealthCheckKind::None),
        discovery: discover.unwrap_or(DiscoveryKind::Static),
    })
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

    // TODO: consistent enforcement of only-known args?
    let args = utils::str_str_args(doc, args)?
        .into_iter()
        .collect::<HashMap<&str, &str>>();

    let proto = match args.get("proto").copied() {
        None => None,
        Some("h1-only") => Some(ALPN::H1),
        Some("h2-only") => Some(ALPN::H2),
        Some("h1-or-h2") => {
            tracing::warn!("accepting 'h1-or-h2' as meaning 'h2-or-h1'");
            Some(ALPN::H2H1)
        }
        Some("h2-or-h1") => Some(ALPN::H2H1),
        Some(other) => {
            return Err(Bad::docspan(
                format!(
                    "'proto' should be one of 'h1-only', 'h2-only', or 'h2-or-h1', found '{other}'"
                ),
                doc,
                node.span(),
            )
            .into());
        }
    };
    let tls_sni = args.get("tls-sni");

    let (tls, sni, alpn) = match (proto, tls_sni) {
        (None, None) | (Some(ALPN::H1), None) => (false, String::new(), ALPN::H1),
        (None, Some(sni)) => (true, sni.to_string(), ALPN::H2H1),
        (Some(_), None) => {
            return Err(
                Bad::docspan("'tls-sni' is required for HTTP2 support", doc, node.span()).into(),
            );
        }
        (Some(p), Some(sni)) => (true, sni.to_string(), p),
    };

    let mut peer = HttpPeer::new(sadd, tls, sni);
    peer.options.alpn = alpn;

    Ok(peer)
}

// services { Service { listeners { ... } } }
fn extract_listener(
    doc: &KdlDocument,
    node: &KdlNode,
    name: &str,
    args: &[KdlEntry],
) -> miette::Result<ListenerConfig> {
    let args = utils::str_value_args(doc, args)?
        .into_iter()
        .collect::<HashMap<&str, &KdlEntry>>();

    // Is this a bindable name?
    if name.parse::<SocketAddr>().is_ok() {
        // Cool: do we have reasonable args for this?
        let cert_path = utils::map_ensure_str(doc, args.get("cert-path").copied())?;
        let key_path = utils::map_ensure_str(doc, args.get("key-path").copied())?;
        let offer_h2 = utils::map_ensure_bool(doc, args.get("offer-h2").copied())?;

        match (cert_path, key_path, offer_h2) {
            // No config? No problem!
            (None, None, None) => Ok(ListenerConfig {
                source: ListenerKind::Tcp {
                    addr: name.to_string(),
                    tls: None,
                    offer_h2: false,
                },
            }),
            // We must have both of cert-path and key-path if both are present
            // ignore "offer-h2" if this is incorrect
            (None, Some(_), _) | (Some(_), None, _) => {
                return Err(Bad::docspan(
                    "'cert-path' and 'key-path' must either BOTH be present, or NEITHER should be present",
                    doc,
                    node.span(),
                )
                .into());
            }
            // We can't offer H2 if we don't have TLS (at least for now, unless we
            // expose H2C settings in pingora)
            (None, None, Some(_)) => {
                return Err(Bad::docspan(
                    "'offer-h2' requires TLS, specify 'cert-path' and 'key-path'",
                    doc,
                    node.span(),
                )
                .into());
            }
            (Some(cpath), Some(kpath), offer_h2) => Ok(ListenerConfig {
                source: ListenerKind::Tcp {
                    addr: name.to_string(),
                    tls: Some(TlsConfig {
                        cert_path: cpath.into(),
                        key_path: kpath.into(),
                    }),
                    // Default to enabling H2 if unspecified
                    offer_h2: offer_h2.unwrap_or(true),
                },
            }),
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
fn extract_system_data(doc: &KdlDocument) -> miette::Result<SystemData> {
    // Get the top level system doc
    let Some(sys) = utils::optional_child_doc(doc, doc, "system") else {
        return Ok(SystemData::default());
    };
    let tps = extract_threads_per_service(doc, sys)?;

    let daemonize = if let Some(n) = sys.get("daemonize") {
        utils::extract_one_bool_arg(doc, n, "daemonize", n.entries())?
    } else {
        false
    };

    let upgrade_socket = if let Some(n) = sys.get("upgrade-socket") {
        let x = utils::extract_one_str_arg(doc, n, "upgrade-socket", n.entries(), |s| {
            Some(PathBuf::from(s))
        })?;
        Some(x)
    } else {
        None
    };

    let pid_file = if let Some(n) = sys.get("pid-file") {
        let x = utils::extract_one_str_arg(doc, n, "pid-file", n.entries(), |s| {
            Some(PathBuf::from(s))
        })?;
        Some(x)
    } else {
        None
    };

    Ok(SystemData {
        threads_per_service: tps,
        daemonize,
        upgrade_socket,
        pid_file,
    })
}

fn extract_threads_per_service(doc: &KdlDocument, sys: &KdlDocument) -> miette::Result<usize> {
    let Some(tps) = sys.get("threads-per-service") else {
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
    /// Helper function for creating a miette span from a given error
    fn docspan(msg: impl Into<String>, doc: &KdlDocument, span: &SourceSpan) -> Self {
        Self {
            error: msg.into(),
            src: doc.to_string(),
            err_span: span.to_owned(),
        }
    }
}
