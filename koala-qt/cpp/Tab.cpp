// Tab implementation — increment 2 of the BrowserWindow port.
//
// At this stage the toolbar actions are stubs: clicking back/forward/
// reload/home prints a `qInfo()` line but doesn't touch any page state,
// because there is no page state yet. Likewise the URL bar `returnPressed`
// handler just logs the entered text. Increment 5 replaces the placeholder
// viewport with a custom widget that paints koala-browser output, at which
// point these slots will call across the cxx bridge into the Rust engine.

#include "Tab.h"

#include "BrowserView.h"
#include "Icons.h"
#include "LocationEdit.h"

#include <QAction>
#include <QDebug>
#include <QPalette>
#include <QVBoxLayout>

namespace koala {

Tab::Tab(QWidget* parent)
    : QWidget(parent)
{
    // Vertical stack: toolbar on top, viewport fills the rest. No
    // margins — we want the toolbar flush against the tab bar above
    // and the viewport flush against the window edges on the sides.
    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);

    build_toolbar();
    layout->addWidget(m_toolbar);

    // Viewport: a `BrowserView` hosts a `koala-browser` engine
    // instance via the cxx bridge. Every fresh tab starts on the
    // built-in landing page.
    m_view = new BrowserView(this);
    m_view->load_landing_page();
    layout->addWidget(m_view, /*stretch=*/1);
}

QString Tab::url_text() const
{
    return m_location_edit->url().toString();
}

void Tab::go_back()
{
    // History navigation still needs a per-tab back/forward stack,
    // which lands when URL navigation is wired up.
    qInfo() << "Tab::go_back (stub)";
}

void Tab::go_forward()
{
    qInfo() << "Tab::go_forward (stub)";
}

void Tab::reload()
{
    // Just re-runs the rendering pipeline at the current viewport
    // size — useful while we're still debugging layout at different
    // window dimensions. Replace with "re-fetch + re-parse" once URL
    // navigation lands.
    m_view->reload_current();
}

void Tab::go_home()
{
    m_view->load_landing_page();
    m_location_edit->clear();
}

void Tab::navigate_to_url_bar_text()
{
    // LocationEdit's own returnPressed handler runs before this one
    // (it's connected first), so by the time we get here
    // `m_location_edit->url()` already reflects the sanitised input.
    // TODO: actually fetch the URL and hand its HTML to
    //       `m_view->load_html`. Needs a non-blocking loader so we
    //       don't hitch the UI thread on slow connections.
    qInfo() << "Tab::navigate_to_url_bar_text (stub) ->"
            << m_location_edit->url().toString();
}

void Tab::focus_location_edit()
{
    m_location_edit->setFocus(Qt::ShortcutFocusReason);
    m_location_edit->selectAll();
}

void Tab::build_toolbar()
{
    m_toolbar = new QToolBar(this);
    m_toolbar->setMovable(false);
    m_toolbar->setFloatable(false);
    // `setIconSize(16)` matches browser-chrome density; the default Qt
    // toolbar icon size is too large for a URL bar toolbar.
    m_toolbar->setIconSize(QSize(16, 16));

    // Icons are drawn programmatically against the current palette's
    // text colour so they match the user's light/dark theme. See
    // `Icons.cpp` for the rendering; these replace Qt's coarse
    // `QStyle::SP_*` pixmap icons.
    auto const icon_color = palette().color(QPalette::WindowText);
    m_back_action = m_toolbar->addAction(
        icons::back(icon_color),
        QStringLiteral("Back"));
    m_forward_action = m_toolbar->addAction(
        icons::forward(icon_color),
        QStringLiteral("Forward"));
    m_reload_action = m_toolbar->addAction(
        icons::reload(icon_color),
        QStringLiteral("Reload"));
    m_home_action = m_toolbar->addAction(
        icons::home(icon_color),
        QStringLiteral("Home"));

    connect(m_back_action, &QAction::triggered, this, &Tab::go_back);
    connect(m_forward_action, &QAction::triggered, this, &Tab::go_forward);
    connect(m_reload_action, &QAction::triggered, this, &Tab::reload);
    connect(m_home_action, &QAction::triggered, this, &Tab::go_home);

    m_location_edit = new LocationEdit(this);
    connect(m_location_edit, &QLineEdit::returnPressed, this, &Tab::navigate_to_url_bar_text);
    m_toolbar->addWidget(m_location_edit);
}

}
