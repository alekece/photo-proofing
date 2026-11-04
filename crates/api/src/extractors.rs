//! Request extractors shared by the handlers: they turn raw request data into validated types
//! (the session cookie, a peer still allowed to attempt a sign-in, a client's private link) before a handler runs.

pub mod attempt;
pub mod client_access;
pub mod identity;
pub mod session_token;
