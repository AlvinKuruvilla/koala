// BrowserWindow — minimal QMainWindow shell.
//
// Increment 1 of the Ladybird `UI/Qt/` port. At this stage all the
// machinery exists (Q_OBJECT, tab widget, menu bar, slots) but the
// individual actions are placeholders. The goal is to prove that the
// moc + cxx-build pipeline compiles and links a signals/slots-enabled
// class into the Rust-driven binary. Later increments will:
//   - Add the toolbar + placeholder URL bar (increment 2)
//   - Port LocationEdit with autocomplete (increment 3)
//   - Port TabBar with per-tab close buttons (increment 4)
//   - Wire koala-browser into the viewport widget (increment 5)

#include "BrowserWindow.h"

#include "Tab.h"
#include "TabBar.h"

#include <QAction>
#include <QKeySequence>
#include <QMenu>
#include <QMenuBar>
#include <QWidget>

namespace koala {

BrowserWindow::BrowserWindow(QWidget* parent)
    : QMainWindow(parent)
    , m_tabs(new TabWidget(this))
{
    setWindowTitle(QStringLiteral("Koala"));
    resize(1280, 800);

    setCentralWidget(m_tabs);

    connect(m_tabs, &TabWidget::tab_close_requested, this, &BrowserWindow::close_tab);
    connect(m_tabs, &TabWidget::new_tab_requested, this, &BrowserWindow::new_tab);

    build_menu_bar();

    // Start with a single empty tab so the window isn't blank on launch.
    new_tab();
}

void BrowserWindow::new_tab()
{
    auto* tab = new Tab(m_tabs);
    m_tabs->add_tab(tab, QStringLiteral("New Tab"));
}

void BrowserWindow::close_tab(int index)
{
    // `TabWidget::remove_tab` handles bounds checking and page
    // deletion itself. We only need to decide whether to close the
    // window when the last tab goes away.
    m_tabs->remove_tab(index);

    if (m_tabs->count() == 0) {
        close();
    }
}

void BrowserWindow::close_current_tab()
{
    int const index = m_tabs->current_index();
    if (index < 0) {
        return;
    }
    close_tab(index);
}

void BrowserWindow::reload_current_tab()
{
    if (auto* tab = m_tabs->current_tab()) {
        tab->reload();
    }
}

void BrowserWindow::back_current_tab()
{
    if (auto* tab = m_tabs->current_tab()) {
        tab->go_back();
    }
}

void BrowserWindow::forward_current_tab()
{
    if (auto* tab = m_tabs->current_tab()) {
        tab->go_forward();
    }
}

void BrowserWindow::focus_current_tab_location_edit()
{
    if (auto* tab = m_tabs->current_tab()) {
        tab->focus_location_edit();
    }
}

void BrowserWindow::build_menu_bar()
{
    // File menu: tab lifecycle + quit.
    auto* file_menu = menuBar()->addMenu(QStringLiteral("&File"));

    auto* new_tab_action = new QAction(QStringLiteral("New &Tab"), this);
    new_tab_action->setShortcuts(QKeySequence::keyBindings(QKeySequence::AddTab));
    connect(new_tab_action, &QAction::triggered, this, &BrowserWindow::new_tab);
    file_menu->addAction(new_tab_action);

    auto* close_tab_action = new QAction(QStringLiteral("&Close Tab"), this);
    close_tab_action->setShortcuts(QKeySequence::keyBindings(QKeySequence::Close));
    connect(close_tab_action, &QAction::triggered, this, &BrowserWindow::close_current_tab);
    file_menu->addAction(close_tab_action);

    file_menu->addSeparator();

    auto* quit_action = new QAction(QStringLiteral("&Quit"), this);
    quit_action->setShortcuts(QKeySequence::keyBindings(QKeySequence::Quit));
    connect(quit_action, &QAction::triggered, this, &BrowserWindow::close);
    file_menu->addAction(quit_action);

    // View menu: reload + URL bar focus.
    auto* view_menu = menuBar()->addMenu(QStringLiteral("&View"));

    auto* reload_action = new QAction(QStringLiteral("&Reload"), this);
    reload_action->setShortcuts(QKeySequence::keyBindings(QKeySequence::Refresh));
    connect(reload_action, &QAction::triggered, this, &BrowserWindow::reload_current_tab);
    view_menu->addAction(reload_action);

    // `QKeySequence::StandardKey` has no entry for "focus location
    // bar". Use Cmd+L on macOS and Ctrl+L elsewhere — the same
    // convention as Safari, Chrome, and Firefox.
    auto* focus_location_action = new QAction(QStringLiteral("Focus &Location Bar"), this);
#ifdef Q_OS_MACOS
    focus_location_action->setShortcut(QKeySequence(QStringLiteral("Meta+L")));
#else
    focus_location_action->setShortcut(QKeySequence(QStringLiteral("Ctrl+L")));
#endif
    connect(focus_location_action, &QAction::triggered, this, &BrowserWindow::focus_current_tab_location_edit);
    view_menu->addAction(focus_location_action);

    // History menu: back / forward / home.
    auto* history_menu = menuBar()->addMenu(QStringLiteral("&History"));

    auto* back_action = new QAction(QStringLiteral("&Back"), this);
    back_action->setShortcuts(QKeySequence::keyBindings(QKeySequence::Back));
    connect(back_action, &QAction::triggered, this, &BrowserWindow::back_current_tab);
    history_menu->addAction(back_action);

    auto* forward_action = new QAction(QStringLiteral("&Forward"), this);
    forward_action->setShortcuts(QKeySequence::keyBindings(QKeySequence::Forward));
    connect(forward_action, &QAction::triggered, this, &BrowserWindow::forward_current_tab);
    history_menu->addAction(forward_action);
}

}
