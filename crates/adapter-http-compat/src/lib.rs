/// Abstraction for HTTP compatibility adapters.
pub trait HttpCompatAdapter {
    fn adapt_request(&self, input: &str) -> String;
}
