//! [OpenType font file](https://learn.microsoft.com/en-us/typography/opentype/spec/otff): where a face's tables are.
//!
//! Reads the TTC header (for a collection) and the face's table
//! directory, and answers "where is table `tag`, and how long is it?".
//! Read in the spec: Data Types, Organization of an OpenType Font, Table
//! Directory, and Font Collections → TTC Header (version 1.0 fields).
//!
//! A table record whose tag breaks the spec's rule for tags (`Tag::new`
//! fails) is skipped, not an error: lookups only ever ask for valid tags,
//! so such a record could never be found anyway.
