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

    connect(m_view, &BrowserView::loadStarted, this, &Tab::on_load_started);
    connect(m_view, &BrowserView::loadFinished, this, &Tab::on_load_finished);
    connect(m_view, &BrowserView::titleChanged, this, &Tab::tabTitleChanged);
}

QString Tab::url_text() const
{
    return m_location_edit->url().toString();
}

void Tab::go_back()
{
    m_view->go_back();
    update_history_actions();
}

void Tab::go_forward()
{
    m_view->go_forward();
    update_history_actions();
}

void Tab::reload()
{
    // If the current page came from a URL, the loader worker
    // re-fetches it; otherwise this just re-renders at the
    // current viewport size.
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
    // Hand the URL off to the async loader; the viewport will
    // repaint when the fetch + parse completes.
    auto const url = m_location_edit->url();
    if (url.isEmpty() || !url.isValid()) {
        return;
    }
    m_view->load_url(url.toString());
}

void Tab::focus_location_edit()
{
    m_location_edit->setFocus(Qt::ShortcutFocusReason);
    m_location_edit->selectAll();
}

void Tab::on_load_started()
{
    if (m_active_loads++ == 0) {
        emit tabLoadStarted();
    }
}

void Tab::on_load_finished()
{
    if (--m_active_loads <= 0) {
        m_active_loads = 0;
        emit tabLoadFinished();
    }
    update_history_actions();
}

void Tab::update_history_actions()
{
    m_back_action->setEnabled(m_view->can_go_back());
    m_forward_action->setEnabled(m_view->can_go_forward());
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

    // Back and Forward start disabled — there's no history until
    // the user navigates somewhere. `update_history_actions` flips
    // them on once `BrowserView::can_go_*` returns true.
    m_back_action->setEnabled(false);
    m_forward_action->setEnabled(false);

    connect(m_back_action, &QAction::triggered, this, &Tab::go_back);
    connect(m_forward_action, &QAction::triggered, this, &Tab::go_forward);
    connect(m_reload_action, &QAction::triggered, this, &Tab::reload);
    connect(m_home_action, &QAction::triggered, this, &Tab::go_home);

    m_location_edit = new LocationEdit(this);
    connect(m_location_edit, &QLineEdit::returnPressed, this, &Tab::navigate_to_url_bar_text);
    m_toolbar->addWidget(m_location_edit);
}

}
