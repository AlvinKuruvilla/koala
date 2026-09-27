//! Problems the engine met while loading a page: CSS it dropped, selectors
//! it cannot match, markup it parsed the wrong way.
//!
//! Engine code calls [`report`] where it gives up on something, and the
//! caller that drives a load collects them with [`take`] once the load
//! is done, as a [`Diagnostics`] value it can print. Repeats are counted
//! rather than stored again, so a declaration dropped on every element it
//! matches shows up once, with the number of times it happened.
//!
//! The collector is thread-local. A page load (parse, cascade, layout,
//! scripts) runs on a single thread, so one thread's collector holds
//! exactly one load's problems, and tabs loading on their own threads in
//! koala-ui do not mix. Code that reports from another thread reports
//! into that thread's collector, which nobody takes; nothing does that
//! today.
//!
//! In quiet mode ([`crate::warning::set_quiet`]) [`report`] does nothing,
//! so `koala --quiet` and the WPT runner pay nothing for it.

use std::cell::RefCell;
use std::fmt;

use koala_std::collections::HashMap;

use crate::warning::is_quiet;

/// One kind of problem, with what is needed to find it in the page.
///
/// Two reports are the same problem when they compare equal, so fields
/// hold what identifies the problem (the property and its value) and not
/// where it occurred.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Diagnostic {
    /// A property Koala does not know. The declaration is dropped.
    UnknownCssProperty {
        /// The property name as written.
        property: String,
    },
    /// A known property whose value Koala could not parse, either because
    /// the value is invalid or because it uses syntax Koala does not
    /// implement (a function like `oklch()`, a keyword it does not know).
    /// The declaration is dropped.
    InvalidCssValue {
        /// The property name as written.
        property: String,
        /// The value as written, before any `var()` substitution.
        value: String,
    },
    /// A CSS-wide keyword (`inherit`, `initial`, `unset`, `revert`,
    /// `revert-layer`) on a property that does not handle it. The
    /// declaration is dropped.
    UnsupportedCssWideKeyword {
        /// The property name as written.
        property: String,
        /// The keyword as written.
        keyword: String,
    },
    /// A value whose `var()` references could not be substituted: the
    /// custom property is undefined and there is no fallback, or the
    /// references form a cycle. Per spec the declaration is then invalid
    /// at computed-value time.
    UnresolvedCssVar {
        /// The property name as written.
        property: String,
        /// The value as written.
        value: String,
    },
    /// A length unit Koala does not implement.
    UnsupportedCssUnit {
        /// The unit as written.
        unit: String,
    },
    /// A `display` keyword Koala does not implement.
    UnsupportedDisplay {
        /// The keyword as written.
        value: String,
    },
    /// `display: contents`, which Koala does not implement. The element
    /// keeps its default box instead of disappearing from the box tree, so
    /// its children lay out inside it rather than in its parent: children
    /// of a flex or grid container wrapped in a `display: contents` element
    /// do not become flex or grid items.
    DisplayContentsNotSupported,
    /// A selector Koala could not parse. Every selector in the rule failed,
    /// so the rule is dropped.
    InvalidSelector {
        /// The rule's selector list as written.
        selector: String,
    },
    /// A pseudo-class Koala does not recognise. The selector never matches.
    UnknownPseudoClass {
        /// The name, without the colon.
        name: String,
    },
    /// A pseudo-class that exists but that Koala does not implement
    /// (`:hover`, `:not()`, `:nth-child()`, ...). The selector never
    /// matches.
    UnsupportedPseudoClass {
        /// The name, without the colon.
        name: String,
    },
    /// A pseudo-element. Koala generates none, so the selector never
    /// matches.
    UnsupportedPseudoElement {
        /// The name, without the colons.
        name: String,
    },
    /// A `<link rel=stylesheet>` whose fetch failed. The page renders as
    /// if the sheet were empty.
    StylesheetLoadFailed {
        /// The link's `href`.
        href: String,
        /// Why the fetch failed.
        error: String,
    },
    /// A parse error in the HTML tokenizer. HTML parse errors are
    /// recoverable by design, so these describe the page, not Koala.
    HtmlTokenizerParseError,
    /// A situation the HTML tree builder recovered from without
    /// implementing it properly.
    HtmlParseWarning {
        /// What happened.
        message: String,
    },
    /// An `<img>` whose fetch or decode failed. The page renders without
    /// it.
    ImageNotLoaded {
        /// The `src` as written.
        src: String,
        /// Why it failed.
        error: String,
    },
    /// An inline `<svg>` whose markup the SVG parser rejected. Its box is
    /// laid out but stays empty.
    InlineSvgNotRendered {
        /// The parser's error.
        error: String,
    },
    /// An image URL fragment (`icons.svg#globe`), which Koala ignores, so
    /// a sprite sheet renders whole.
    ImageUrlFragmentIgnored {
        /// The `src` as written.
        src: String,
    },
    /// An image URL query string, which Koala ignores.
    ImageUrlQueryIgnored {
        /// The `src` as written.
        src: String,
    },
}

impl Diagnostic {
    /// The engine area the problem belongs to, for grouping in output.
    #[must_use]
    pub const fn area(&self) -> &'static str {
        match self {
            Self::UnknownCssProperty { .. }
            | Self::InvalidCssValue { .. }
            | Self::UnsupportedCssWideKeyword { .. }
            | Self::UnresolvedCssVar { .. }
            | Self::UnsupportedCssUnit { .. }
            | Self::UnsupportedDisplay { .. }
            | Self::DisplayContentsNotSupported
            | Self::InvalidSelector { .. }
            | Self::UnknownPseudoClass { .. }
            | Self::UnsupportedPseudoClass { .. }
            | Self::UnsupportedPseudoElement { .. }
            | Self::StylesheetLoadFailed { .. } => "CSS",
            Self::HtmlTokenizerParseError | Self::HtmlParseWarning { .. } => "HTML",
            Self::ImageNotLoaded { .. }
            | Self::InlineSvgNotRendered { .. }
            | Self::ImageUrlFragmentIgnored { .. }
            | Self::ImageUrlQueryIgnored { .. } => "image",
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCssProperty { property } => {
                write!(f, "unknown property '{property}'")
            }
            Self::InvalidCssValue { property, value } => {
                write!(f, "dropped '{property}: {value}': value not understood")
            }
            Self::UnsupportedCssWideKeyword { property, keyword } => {
                write!(f, "dropped '{property}: {keyword}': '{keyword}' not supported here")
            }
            Self::UnresolvedCssVar { property, value } => {
                write!(f, "dropped '{property}: {value}': var() did not resolve")
            }
            Self::UnsupportedCssUnit { unit } => write!(f, "unsupported unit '{unit}'"),
            Self::UnsupportedDisplay { value } => write!(f, "unsupported display value '{value}'"),
            Self::DisplayContentsNotSupported => write!(
                f,
                "display: contents not supported: the element keeps its box, so its children \
                 are not laid out as children of its parent (flex and grid items under it are wrong)"
            ),
            Self::InvalidSelector { selector } => {
                write!(f, "rule dropped: could not parse selector '{selector}'")
            }
            Self::UnknownPseudoClass { name } => {
                write!(f, "unknown pseudo-class ':{name}': selector never matches")
            }
            Self::UnsupportedPseudoClass { name } => {
                write!(f, "pseudo-class ':{name}' not supported: selector never matches")
            }
            Self::UnsupportedPseudoElement { name } => {
                write!(f, "pseudo-element '::{name}' not supported: selector never matches")
            }
            Self::StylesheetLoadFailed { href, error } => {
                write!(f, "stylesheet '{href}' not loaded: {error}")
            }
            Self::HtmlTokenizerParseError => write!(f, "tokenizer parse error"),
            Self::HtmlParseWarning { message } => write!(f, "{message}"),
            Self::ImageNotLoaded { src, error } => write!(f, "image '{src}' not shown: {error}"),
            Self::InlineSvgNotRendered { error } => write!(f, "inline <svg> not drawn: {error}"),
            Self::ImageUrlFragmentIgnored { src } => {
                write!(f, "ignoring fragment in '{src}': sprite sheets not supported")
            }
            Self::ImageUrlQueryIgnored { src } => {
                write!(f, "ignoring query string in '{src}'")
            }
        }
    }
}

/// The problems collected during one load, each with how often it
/// happened, in the order first seen.
#[derive(Debug, Default, Clone)]
pub struct Diagnostics {
    entries: Vec<(Diagnostic, u32)>,
    index: HashMap<Diagnostic, usize>,
}

impl Diagnostics {
    /// Record one occurrence of `diagnostic`.
    ///
    /// # Time complexity
    ///
    /// Expected *O*(1), plus hashing the diagnostic's strings.
    pub fn add(&mut self, diagnostic: Diagnostic) {
        if let Some(&i) = self.index.get(&diagnostic) {
            self.entries[i].1 += 1;
            return;
        }
        let _ = self.index.insert(diagnostic.clone(), self.entries.len());
        self.entries.push((diagnostic, 1));
    }

    /// Whether nothing was reported.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The number of distinct problems.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Each distinct problem with its occurrence count, in the order first
    /// seen.
    ///
    /// # Time complexity
    ///
    /// *O*(1) to create; *O*(*n*) to exhaust.
    pub fn iter(&self) -> impl Iterator<Item = (&Diagnostic, u32)> {
        self.entries.iter().map(|(d, n)| (d, *n))
    }
}

/// One line per problem, grouped by area, with a count where it happened
/// more than once:
///
/// ```text
///   CSS    unknown property 'aspect-ratio' (x2)
///   HTML   <svg> parsed as HTML: ...
/// ```
impl fmt::Display for Diagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_lines(f, false)
    }
}

impl Diagnostics {
    /// The same lines as `Display`, coloured when stderr is a terminal that
    /// takes colour (and `NO_COLOR` is unset): area labels yellow, counts
    /// dimmed. For output that goes to stderr.
    #[must_use]
    pub fn to_stderr_string(&self) -> String {
        struct Coloured<'a>(&'a Diagnostics);
        impl fmt::Display for Coloured<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.write_lines(f, true)
            }
        }
        Coloured(self).to_string()
    }

    fn write_lines(&self, f: &mut fmt::Formatter<'_>, colour: bool) -> fmt::Result {
        use owo_colors::{OwoColorize, Stream::Stderr};

        let mut areas: Vec<&'static str> = Vec::new();
        for (diagnostic, _) in &self.entries {
            if !areas.contains(&diagnostic.area()) {
                areas.push(diagnostic.area());
            }
        }
        for area in areas {
            let label = format!("{area:<6}");
            for (diagnostic, count) in self.entries.iter().filter(|(d, _)| d.area() == area) {
                if colour {
                    write!(f, "  {} {diagnostic}", label.if_supports_color(Stderr, |t| t.yellow()))?;
                } else {
                    write!(f, "  {label} {diagnostic}")?;
                }
                if *count > 1 {
                    let count = format!("(x{count})");
                    if colour {
                        write!(f, " {}", count.if_supports_color(Stderr, |t| t.dimmed()))?;
                    } else {
                        write!(f, " {count}")?;
                    }
                }
                writeln!(f)?;
            }
        }
        Ok(())
    }
}

thread_local! {
    static COLLECTOR: RefCell<Diagnostics> = RefCell::new(Diagnostics::default());
}

/// Record a problem in this thread's collector.
///
/// Takes a closure so the diagnostic's strings are only built when they
/// will be kept: in quiet mode the closure never runs.
pub fn report(make: impl FnOnce() -> Diagnostic) {
    if is_quiet() {
        return;
    }
    COLLECTOR.with(|c| c.borrow_mut().add(make()));
}

/// Take everything this thread has collected, leaving the collector empty.
///
/// Called at the start of a load to discard anything left over, and at the
/// end to hand the load's problems to the caller.
#[must_use]
pub fn take() -> Diagnostics {
    COLLECTOR.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unknown(property: &str) -> Diagnostic {
        Diagnostic::UnknownCssProperty { property: property.to_string() }
    }

    /// A repeat bumps the count of the first report instead of adding an
    /// entry, and order stays first-seen.
    #[test]
    fn repeats_are_counted() {
        let mut diagnostics = Diagnostics::default();
        diagnostics.add(unknown("aspect-ratio"));
        diagnostics.add(Diagnostic::HtmlTokenizerParseError);
        diagnostics.add(unknown("aspect-ratio"));
        let entries: Vec<_> = diagnostics.iter().collect();
        assert_eq!(
            entries,
            [(&unknown("aspect-ratio"), 2), (&Diagnostic::HtmlTokenizerParseError, 1)]
        );
    }

    /// Output groups by area in first-seen order and marks repeats.
    #[test]
    fn display_groups_by_area() {
        let mut diagnostics = Diagnostics::default();
        diagnostics.add(unknown("aspect-ratio"));
        diagnostics.add(Diagnostic::HtmlTokenizerParseError);
        diagnostics.add(unknown("place-items"));
        diagnostics.add(unknown("place-items"));
        assert_eq!(
            diagnostics.to_string(),
            "  CSS    unknown property 'aspect-ratio'\n\
             \x20 CSS    unknown property 'place-items' (x2)\n\
             \x20 HTML   tokenizer parse error\n"
        );
    }

    /// `take` hands over what was reported and empties the collector.
    #[test]
    fn take_empties_the_collector() {
        report(|| unknown("aspect-ratio"));
        assert_eq!(take().len(), 1);
        assert!(take().is_empty());
    }
}
