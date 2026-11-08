//! Request extractors shared by the handlers: they turn raw request data into validated types
//! (the session cookie, a client's private link) before a handler runs.

pub mod client_access;
pub mod identity;
pub mod session_token;
