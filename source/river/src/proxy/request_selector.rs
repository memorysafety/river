use pingora_proxy::Session;

use super::RiverContext;

pub type RequestSelector = for<'a> fn(&'a mut [u8], &mut RiverContext, &mut Session) -> &'a [u8];

pub fn null_selector<'a>(
    _buf: &'a mut [u8],
    _ctxt: &mut RiverContext,
    _ses: &mut Session,
) -> &'a [u8] {
    &[]
}
