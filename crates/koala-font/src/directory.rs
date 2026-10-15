//! [OpenType font file](https://learn.microsoft.com/en-us/typography/opentype/spec/otff): where a face's tables are.
//!
//! Reads the TTC header (for a collection) and the face's table
//! directory, and answers "where is table `tag`, and how long is it?".
//! Read in the spec: Data Types, Organization of an OpenType Font, Table
//! Directory, and Font Collections → TTC Header (version 1.0 fields).
