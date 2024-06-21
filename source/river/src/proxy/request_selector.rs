use std::io::Write;

use pingora_proxy::Session;

use super::RiverContext;

/// A function used to determine the "key" to use for the selection process.
///
/// The function may choose an existing series of bytes, or may format into
/// the RiverContext.selector_buf field, using `write!` or similar formatting
/// options.
///
/// TODO: Should I just do `Cow<'a, [u8]>` instead of providing a buffer? The intent is
/// to avoid allocations on every select (reusing and growing one instead), but this might
/// have "weird" mem-leaky characteristics
pub type RequestSelector = for<'a> fn(&'a mut RiverContext, &'a mut Session) -> &'a [u8];

/// Null selector, useful when using "Random" or "RoundRobin" selection and this key is not used
///
/// Performs no formatting
pub fn null_selector<'a>(_ctxt: &'a mut RiverContext, _ses: &'a mut Session) -> &'a [u8] {
    &[]
}

/// Basic selector that looks at ONLY the URI of the request as the input key
///
/// Peforms no formatting
pub fn uri_path_selector<'a>(_ctxt: &'a mut RiverContext, ses: &'a mut Session) -> &'a [u8] {
    ses.req_header().uri.path().as_bytes()
}

/// Selector that uses the source address (if available) and the URI of the request as the input key
///
/// Performs formatting into the selector buf
pub fn source_addr_and_uri_path_selector<'a>(
    ctxt: &'a mut RiverContext,
    ses: &'a mut Session,
) -> &'a [u8] {
    write!(
        &mut ctxt.selector_buf,
        "{:?}:{}",
        ses.as_downstream().client_addr(),
        ses.req_header().uri.path(),
    )
    .expect("Formatting into a Vec<u8> should never fail");

    ctxt.selector_buf.as_slice()
}
