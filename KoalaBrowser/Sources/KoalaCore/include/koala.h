#ifndef KOALA_H
#define KOALA_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Opaque handle to a parsed DOM document
typedef struct KoalaDocument KoalaDocument;

/// Parse HTML string and return a handle to the document.
/// Returns NULL on error.
///
/// @param html A null-terminated C string containing HTML
/// @return A pointer to the parsed document, or NULL on error
/// @note The returned pointer must be freed with koala_document_free()
KoalaDocument* koala_parse_html(const char* html);

/// Free a document handle.
///
/// @param doc A document pointer returned by koala_parse_html(), or NULL
void koala_document_free(KoalaDocument* doc);

/// Get the document as a JSON string representation.
/// Returns NULL on error.
///
/// @param doc A document pointer returned by koala_parse_html()
/// @return A JSON string representation of the document, or NULL on error
/// @note The returned string must be freed with koala_string_free()
char* koala_document_to_json(const KoalaDocument* doc);

/// Free a string returned by Koala functions.
///
/// @param s A string pointer returned by a Koala function, or NULL
void koala_string_free(char* s);

/// Get the number of child nodes for a document.
///
/// @param doc A document pointer returned by koala_parse_html()
/// @return The number of child nodes
size_t koala_document_child_count(const KoalaDocument* doc);

#ifdef __cplusplus
}
#endif

#endif /* KOALA_H */
