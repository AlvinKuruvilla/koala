//! What a fetch asks for.

/// [Fetch § 2.2.5 Requests](https://fetch.spec.whatwg.org/#requests)
///
/// "A request has an associated URL." It also has a destination, which
/// decides the headers a browser sends with it. Headers the spec derives
/// from other request state (`Referer`, `Origin`, cookies) become fields
/// here when koala sends them.
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    /// The URL to fetch, as the loader resolved it.
    pub url: &'a str,
    /// What the response will be used for.
    pub destination: Destination,
}

impl<'a> Request<'a> {
    /// A request for `url`, to be used as `destination`.
    #[must_use]
    pub const fn new(url: &'a str, destination: Destination) -> Self {
        Self { url, destination }
    }
}

/// [Fetch § 2.2.5 Requests](https://fetch.spec.whatwg.org/#concept-request-destination)
///
/// "A request has an associated destination", the kind of resource being
/// fetched. Only the destinations koala fetches are listed; the spec has
/// more (`font`, `json`, `worker`, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    /// The page itself (`"document"`).
    Document,
    /// A `<link rel=stylesheet>` (`"style"`).
    Style,
    /// A `<script src>` (`"script"`).
    Script,
    /// An `<img src>` (`"image"`).
    Image,
}

impl Destination {
    /// The `Accept` header value for this destination.
    ///
    /// [Fetch § 4 Fetching](https://fetch.spec.whatwg.org/#fetching)
    ///
    /// "If request's header list does not contain `Accept`, then: Let value
    /// be `*/*`. [...] Otherwise, the user agent should set value to the
    /// first matching statement, if any, switching on request's
    /// destination: "document" "frame" "iframe" the document `Accept`
    /// header value "image"
    /// `image/png,image/svg+xml,image/*;q=0.8,*/*;q=0.5` [...] "style"
    /// `text/css,*/*;q=0.1` [...]"
    ///
    /// "The document `Accept` header value is
    /// `text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8`."
    ///
    /// Scripts match no statement and keep `*/*`.
    #[must_use]
    pub const fn accept(self) -> &'static str {
        match self {
            Self::Document => "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            Self::Image => "image/png,image/svg+xml,image/*;q=0.8,*/*;q=0.5",
            Self::Style => "text/css,*/*;q=0.1",
            Self::Script => "*/*",
        }
    }
}

/// The language the user reads, sent as `Accept-Language`.
///
/// [Fetch § 4 Fetching](https://fetch.spec.whatwg.org/#fetching)
///
/// "If request's header list does not contain `Accept-Language`, then user
/// agents should append (`Accept-Language`, an appropriate header value)
/// to request's header list."
///
/// A user-agent setting, not a property of one request, so it lives on
/// [`DefaultSender`](crate::DefaultSender). Only US English exists until
/// koala has a language setting; servers that localize then serve English,
/// and those that vary on the header serve what a browser would.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    /// American English.
    #[default]
    EnUs,
}

impl Language {
    /// The `Accept-Language` header value: this language, then its
    /// language family at lower priority, as browsers send it.
    #[must_use]
    pub const fn accept_language(self) -> &'static str {
        match self {
            Self::EnUs => "en-US,en;q=0.9",
        }
    }
}
