// BrowserWindow — the top-level QMainWindow for the koala browser UI.
//
// Modeled on Ladybird's `UI/Qt/BrowserWindow.h`, but stripped to the bones:
// - No LibWeb/LibWebView/AK dependencies
// - No WebContentView (the rendered page surface is stubbed for now)
// - No fullscreen mode, bookmarks, find-in-page, or hamburger menu yet
//
// The intent is to grow back toward feature parity with Ladybird's
// BrowserWindow one increment at a time, replacing stubs with real
// behaviour backed by koala-browser via the cxx bridge.

#pragma once

#include <QMainWindow>

namespace koala {

class TabWidget;

class BrowserWindow : public QMainWindow {
    Q_OBJECT

public:
    explicit BrowserWindow(QWidget* parent = nullptr);
    ~BrowserWindow() override = default;

public slots:
    // Creates a new empty tab and activates it. At this stage the tab body
    // is a placeholder `QWidget` — once the viewport is wired up it will
    // host a koala-browser page.
    void new_tab();

    // Closes the tab at `index`. Does nothing if the index is out of
    // range. Mirrors Ladybird's `request_to_close_tab`.
    void close_tab(int index);

    // Closes whichever tab is currently active. Bound to ⌘W / Ctrl+W
    // via `QKeySequence::Close`. Falls through to `close_tab(index)`.
    void close_current_tab();

    // The following slots all forward to the currently active `Tab`.
    // Each bails out silently when there is no current tab (for
    // instance, during the brief window between closing the last tab
    // and the window closing itself).
    void reload_current_tab();
    void back_current_tab();
    void forward_current_tab();
    void focus_current_tab_location_edit();

private:
    // Builds File / View / History menus on `menuBar()` and registers
    // the OS-standard shortcuts for each action. Kept simple for now —
    // most actions log via `qInfo()` until navigation is wired up in
    // increment 5.
    void build_menu_bar();

    TabWidget* m_tabs { nullptr };
};

}
