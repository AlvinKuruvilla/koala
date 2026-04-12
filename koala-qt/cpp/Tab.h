// Tab — one browser tab's UI: toolbar on top, viewport below.
//
// Modeled on Ladybird's `UI/Qt/Tab.h`, stripped to the essentials:
// - QToolBar with back / forward / reload / home actions
// - QLineEdit as the URL bar (a `LocationEdit` port will replace this
//   in increment 3)
// - A placeholder `QWidget` where koala-browser's rendered output will
//   eventually be painted (increment 5)
//
// Each tab owns its own toolbar so back/forward stacks are per-tab, the
// same architecture Ladybird uses.

#pragma once

#include <QToolBar>
#include <QWidget>

class QAction;

namespace koala {

class BrowserView;
class LocationEdit;

class Tab : public QWidget {
    Q_OBJECT

public:
    explicit Tab(QWidget* parent = nullptr);
    ~Tab() override = default;

    // The text currently shown in the URL bar. Used by BrowserWindow to
    // keep the tab title in sync once navigation is wired up.
    QString url_text() const;

public slots:
    void go_back();
    void go_forward();
    void reload();
    void go_home();
    void navigate_to_url_bar_text();

    // Moves keyboard focus to the URL bar and selects its contents,
    // matching the behaviour of ⌘L / Ctrl+L in mainstream browsers.
    void focus_location_edit();

signals:
    // Emitted when this tab enters the loading state (first load
    // request became in-flight). `TabWidget` listens for this to
    // start showing a spinner on this tab's bar entry.
    void tabLoadStarted();

    // Emitted when the last in-flight load for this tab completes.
    // `TabWidget` listens for this to clear the spinner.
    void tabLoadFinished();

    // Forwards `BrowserView::titleChanged` for the `TabWidget` to
    // use as the tab bar label. Empty string when the current
    // document has no `<title>` — callers should fall back to a
    // default like "New Tab" in that case.
    void tabTitleChanged(QString const& title);

private slots:
    // Wired to `BrowserView::loadStarted`. Increments an in-flight
    // load counter and emits `tabLoadStarted` when the counter
    // leaves zero. The counter prevents overlapping navigations
    // from making the spinner flicker on/off.
    void on_load_started();

    // Wired to `BrowserView::loadFinished`. Decrements the counter
    // and emits `tabLoadFinished` when it hits zero. Also refreshes
    // the Back/Forward action enabled state so the toolbar reflects
    // the new history position.
    void on_load_finished();

private:
    // Refreshes the enabled state of the Back and Forward toolbar
    // actions from `BrowserView::can_go_back` /
    // `can_go_forward`. Called after every `go_back` / `go_forward`
    // / `on_load_finished` so the chrome tracks the history stack.
    void update_history_actions();

private:
    void build_toolbar();

    QToolBar* m_toolbar { nullptr };
    LocationEdit* m_location_edit { nullptr };
    BrowserView* m_view { nullptr };
    // Count of in-flight loads. Debounces the `tabLoadStarted` /
    // `tabLoadFinished` signal pair across overlapping navigations
    // so the tab spinner doesn't flicker on/off.
    int m_active_loads { 0 };

    QAction* m_back_action { nullptr };
    QAction* m_forward_action { nullptr };
    QAction* m_reload_action { nullptr };
    QAction* m_home_action { nullptr };
};

}
