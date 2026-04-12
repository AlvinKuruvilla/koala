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

private:
    void build_toolbar();

    QToolBar* m_toolbar { nullptr };
    LocationEdit* m_location_edit { nullptr };
    BrowserView* m_view { nullptr };

    QAction* m_back_action { nullptr };
    QAction* m_forward_action { nullptr };
    QAction* m_reload_action { nullptr };
    QAction* m_home_action { nullptr };
};

}
