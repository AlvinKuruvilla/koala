// LocationEdit implementation.
//
// The class is intentionally a close structural match to Ladybird's
// `UI/Qt/LocationEdit.cpp` minus autocomplete and settings. When koala
// grows those subsystems the re-add should be mechanical.

#include "LocationEdit.h"

#include <QCoreApplication>
#include <QFocusEvent>
#include <QInputMethodEvent>
#include <QKeyEvent>
#include <QList>
#include <QPalette>
#include <QTextCharFormat>
#include <QTimer>
#include <QUrl>

namespace koala {

LocationEdit::LocationEdit(QWidget* parent)
    : QLineEdit(parent)
{
    setPlaceholderText(QStringLiteral("Enter web address"));
    setClearButtonEnabled(true);

    // On returnPressed, sanitize the input and cache the result as
    // `m_url` so the owning `Tab` can pick it up in its own slot.
    // Qt fires slots in connection order, and this connection runs
    // before Tab's, so `tab->url()` sees the committed value.
    connect(this, &QLineEdit::returnPressed, this, [this]() {
        if (text().isEmpty()) {
            return;
        }
        clearFocus();
        if (auto url = normalize_input(text()); url.isValid()) {
            set_url(std::move(url));
        }
    });

    // Re-highlight on every edit so the host stays emphasised as the
    // user types. QLineEdit emits textChanged for both user edits and
    // programmatic setText calls, which is what we want.
    connect(this, &QLineEdit::textChanged, this, &LocationEdit::highlight_location);

    // TODO: wire up an autocomplete completer once koala has a search
    //       suggestion backend. Ladybird uses a LibWebView-backed
    //       `Autocomplete` (a QCompleter subclass) here.
}

void LocationEdit::set_url(QUrl url)
{
    m_url = std::move(url);
    setText(m_url.toString());
    setCursorPosition(0);
}

void LocationEdit::focusInEvent(QFocusEvent* event)
{
    QLineEdit::focusInEvent(event);
    highlight_location();

    // Defer selectAll to the next event-loop tick so Qt's own
    // focus-in cursor-placement logic has settled first. Matches
    // Ladybird's behaviour exactly.
    if (event->reason() != Qt::PopupFocusReason) {
        QTimer::singleShot(0, this, &QLineEdit::selectAll);
    }
}

void LocationEdit::focusOutEvent(QFocusEvent* event)
{
    QLineEdit::focusOutEvent(event);

    if (event->reason() != Qt::PopupFocusReason) {
        setCursorPosition(0);
        highlight_location();
    }
}

void LocationEdit::keyPressEvent(QKeyEvent* event)
{
    // Escape abandons the in-progress edit and restores the committed
    // URL. If there is no committed URL yet just clear the field.
    if (event->key() == Qt::Key_Escape) {
        setText(m_url.isEmpty() ? QString() : m_url.toString());
        clearFocus();
        return;
    }
    QLineEdit::keyPressEvent(event);
}

void LocationEdit::highlight_location()
{
    // Build a list of text-format attributes that dim the scheme/path
    // while leaving the host at full palette text colour. We send them
    // via a `QInputMethodEvent` because `QLineEdit` does not expose a
    // direct rich-text API — this is the same trick Ladybird uses.

    auto const current = text();
    auto const parsed = QUrl::fromUserInput(current);

    QList<QInputMethodEvent::Attribute> attributes;

    // Only apply dimming if we have a recognisable host to emphasise.
    // Otherwise leave the field at default styling.
    if (parsed.isValid() && !parsed.host().isEmpty()) {
        auto dim_color = QPalette().color(QPalette::Text);
        dim_color.setAlpha(127);
        QTextCharFormat dim;
        dim.setForeground(dim_color);

        QTextCharFormat bright;
        bright.setForeground(QPalette().color(QPalette::Text));

        // Locate the host within the current text. We can't rely on
        // `parsed.host()` offsets because `fromUserInput` may have
        // added a scheme. Do a simple substring search instead.
        //
        // Qt 6 returns `qsizetype` (a signed 64-bit integer) from
        // `length()` and `indexOf()`, but `QInputMethodEvent::Attribute`
        // takes `int`. Cast narrowly at the boundary.
        auto const host = parsed.host();
        int const host_start = static_cast<int>(current.indexOf(host));
        int const host_length = static_cast<int>(host.length());
        int const current_length = static_cast<int>(current.length());

        if (host_start >= 0) {
            int const host_end = host_start + host_length;
            int const cursor = cursorPosition();

            // Dim the prefix (scheme and subdomain up to the host).
            if (host_start > 0) {
                attributes.append({
                    QInputMethodEvent::TextFormat,
                    -cursor,
                    host_start,
                    dim,
                });
            }

            // Highlight the host itself.
            attributes.append({
                QInputMethodEvent::TextFormat,
                host_start - cursor,
                host_length,
                bright,
            });

            // Dim the suffix (path, query, fragment).
            if (host_end < current_length) {
                attributes.append({
                    QInputMethodEvent::TextFormat,
                    host_end - cursor,
                    current_length - host_end,
                    dim,
                });
            }
        }
    }

    QInputMethodEvent event(QString(), attributes);
    QCoreApplication::sendEvent(this, &event);
}

QUrl LocationEdit::normalize_input(QString const& text)
{
    // QUrl::fromUserInput handles:
    //   - bare hostnames (`example.com` → `https://example.com`)
    //   - IP literals (`127.0.0.1:8080` → `http://127.0.0.1:8080`)
    //   - file paths (`/etc/hosts` → `file:///etc/hosts`)
    //   - already-valid URLs (pass through)
    // TODO: once koala has a settings system, non-URL input should
    //       fall back to a search-engine URL rather than returning
    //       invalid. Ladybird uses `WebView::sanitize_url` here.
    return QUrl::fromUserInput(text);
}

}
