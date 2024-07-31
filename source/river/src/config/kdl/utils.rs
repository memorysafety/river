//! Various ad-hoc KDL document parsers used

use super::{Bad, OptExtParse};
use kdl::{KdlDocument, KdlEntry, KdlNode};
use std::collections::HashMap;

/// Get the child document with a given name, or return an error
///
/// For example, get the "service" doc within the top level doc
pub(crate) fn required_child_doc<'a>(
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

/// Like `required_child_doc`, but allowed to be missing
pub(crate) fn optional_child_doc<'a>(
    _doc: &KdlDocument,
    here: &'a KdlDocument,
    name: &str,
) -> Option<&'a KdlDocument> {
    let node = here.get(name)?;

    node.children()
}

/// Get 0..N children nodes that are themselves named nodes with children
///
/// For example: All the named services in the `services` block
pub(crate) fn wildcard_argless_child_docs<'a>(
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
pub(crate) fn data_nodes<'a>(
    _doc: &KdlDocument,
    here: &'a KdlDocument,
) -> miette::Result<Vec<(&'a KdlNode, &'a str, &'a [KdlEntry])>> {
    let mut out = vec![];
    for node in here.nodes() {
        out.push((node, node.name().value(), node.entries()));
    }
    Ok(out)
}

/// Useful for collecting all arguments as str:str key pairs
pub(crate) fn str_str_args<'a>(
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

/// Useful for collecting all arguments as str:Value key pairs
///
/// KdlEntry is returned instead of KdlValue to allow for retaining the
/// span for error messages
pub(crate) fn str_value_args<'a>(
    doc: &KdlDocument,
    args: &'a [KdlEntry],
) -> miette::Result<Vec<(&'a str, &'a KdlEntry)>> {
    let mut out = vec![];
    for arg in args {
        let name =
            arg.name()
                .map(|a| a.value())
                .or_bail("arguments should be named", doc, arg.span())?;

        out.push((name, arg));
    }
    Ok(out)
}

/// If the argument exists, ensure it is a str
///
/// Useful with [`str_value_args()`].
pub(crate) fn map_ensure_str<'a>(doc: &'_ KdlDocument, val: Option<&'a KdlEntry>) -> miette::Result<Option<&'a str>> {
    let Some(v) = val else {
        return Ok(None);
    };
    match v.value().as_string() {
        Some(vas) => Ok(Some(vas)),
        None => {
            Err(Bad::docspan("Expected string argument", doc, v.span()).into())
        }
    }
}

/// If the argument exists, ensure it is a bool
///
/// Useful with [`str_value_args()`].
pub(crate) fn map_ensure_bool(doc: &KdlDocument, val: Option<&KdlEntry>) -> miette::Result<Option<bool>> {
    let Some(v) = val else {
        return Ok(None);
    };
    match v.value().as_bool() {
        Some(vas) => Ok(Some(vas)),
        None => {
            Err(Bad::docspan("Expected bool argument", doc, v.span()).into())
        }
    }
}

/// Extract a single un-named string argument, like `discovery "Static"`
pub(crate) fn extract_one_str_arg<T, F: FnOnce(&str) -> Option<T>>(
    doc: &KdlDocument,
    node: &KdlNode,
    name: &str,
    args: &[KdlEntry],
    f: F,
) -> miette::Result<T> {
    match args {
        [one] => one.value().as_string().and_then(f),
        _ => None,
    }
    .or_bail(format!("Incorrect argument for '{name}'"), doc, node.span())
}

/// Extract a single un-named bool argument, like `daemonize true`
pub(crate) fn extract_one_bool_arg(
    doc: &KdlDocument,
    node: &KdlNode,
    name: &str,
    args: &[KdlEntry],
) -> miette::Result<bool> {
    match args {
        [one] => one.value().as_bool(),
        _ => None,
    }
    .or_bail(format!("Incorrect argument for '{name}'"), doc, node.span())
}

/// Like `extract_one_str_arg`, but with bonus "str:str" key/val pairs
///
/// `selection "Ketama" key="UriPath"`
pub(crate) fn extract_one_str_arg_with_kv_args<T, F: FnOnce(&str) -> Option<T>>(
    doc: &KdlDocument,
    node: &KdlNode,
    name: &str,
    args: &[KdlEntry],
    f: F,
) -> miette::Result<(T, HashMap<String, String>)> {
    let (first, rest) =
        args.split_first()
            .or_bail(format!("Missing arguments for '{name}'"), doc, node.span())?;
    let first = first.value().as_string().and_then(f).or_bail(
        format!("Incorrect argument for '{name}'"),
        doc,
        node.span(),
    )?;
    let mut kvs = HashMap::new();
    rest.iter().try_for_each(|arg| -> miette::Result<()> {
        let key = arg
            .name()
            .or_bail("Should be a named argument", doc, arg.span())?
            .value();
        let val = arg
            .value()
            .as_string()
            .or_bail("Should be a string value", doc, arg.span())?;
        kvs.insert(key.to_string(), val.to_string());
        Ok(())
    })?;

    Ok((first, kvs))
}
