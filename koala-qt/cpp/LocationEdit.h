// LocationEdit — browser URL bar, a QLineEdit subclass.
//
// Modeled on Ladybird's `UI/Qt/LocationEdit.h`. Notable trims from the
// Ladybird original:
//
// - No `Autocomplete` / `QCompleter`: Ladybird's completer queries a
//   search-engine suggestion backend via LibWebView. Koala has no
//   settings system or HTTP client for this yet, so the widget is a
//   plain QLineEdit with no dropdown. A TODO in the cpp file records
//   where to plug autocomplete in once a backend exists.
//
// - No `SettingsObserver`: the placeholder text is fixed at "Enter web
//   address". Ladybird dynamically updates it when the search engine
//   changes.
//
// - `m_url` is stored as `QUrl` rather than `URL::URL` (the LibURL
//   type). `QUrl` is Qt-native and loses nothing we currently care
//   about — once koala-browser has a richer URL type we can bridge it
//   over cxx.
//
// The rest is faithful: focus-in selects all, focus-out collapses the
// cursor, Escape restores the committed URL, and `highlight_location`
// dims the scheme and path to emphasise the host.

#pragma once

#include <QLineEdit>
#include <QUrl>

class QFocusEvent;
class QKeyEvent;

namespace koala {

class LocationEdit : public QLineEdit {
    Q_OBJECT

public:
    explicit LocationEdit(QWidget* parent = nullptr);
    ~LocationEdit() override = default;

    // The URL most recently committed via returnPressed / `set_url`.
    // May differ from `text()` while the user is editing.
    QUrl const& url() const { return m_url; }

    // Replaces the committed URL and updates the visible text. Called
    // by the owning `Tab` after a navigation completes.
    void set_url(QUrl url);

protected:
    void focusInEvent(QFocusEvent* event) override;
    void focusOutEvent(QFocusEvent* event) override;
    void keyPressEvent(QKeyEvent* event) override;

private:
    // Dims the scheme and path, leaving the host at full opacity.
    // Called on every text change and on focus transitions.
    void highlight_location();

    // Turns user-entered text into a committed URL. Uses
    // `QUrl::fromUserInput`, which handles scheme-less input like
    // `example.com` or `192.168.0.1:8080` sensibly. Returns an
    // invalid `QUrl` if the input can't be interpreted.
    static QUrl normalize_input(QString const& text);

    QUrl m_url;
};

}
