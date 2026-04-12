// TabBar / TabWidget implementation. See header for the port rationale.

#include "TabBar.h"

#include "Icons.h"
#include "Tab.h"

#include <QEvent>
#include <QHBoxLayout>
#include <QMouseEvent>
#include <QPalette>
#include <QResizeEvent>
#include <QTimer>
#include <QVBoxLayout>

#include <algorithm>

namespace {
// ~12 fps rotation. Each tick advances the angle by 30° so a full
// revolution takes ~1 second — matching the Chrome/Safari loading
// spinner cadence. Any faster and the motion distracts; any slower
// and the user wonders if the browser is stuck.
constexpr int kSpinnerIntervalMs = 80;
constexpr int kSpinnerStepDegrees = 30;
}

namespace koala {

TabBar::TabBar(TabWidget* parent)
    : QTabBar(parent)
    , m_tab_widget(parent)
{
}

void TabBar::set_available_width(int width)
{
    // Skip the relayout work if nothing changed — `updateGeometry` is
    // cheap but not free, and we hit this path on every resize event.
    if (m_available_width != width) {
        m_available_width = width;
        updateGeometry();
    }
}

QSize TabBar::tabSizeHint(int /*index*/) const
{
    // Base the hint on the parent class's heuristic for height and
    // other metrics, then override the width.
    auto hint = QTabBar::tabSizeHint(0);

    if (auto const tab_count = count(); tab_count > 0) {
        int const total_width = m_available_width > 0 ? m_available_width : this->width();
        int width = total_width / tab_count;
        // Clamp to [128, 225] so tabs stay readable but don't hog space.
        width = std::min(225, width);
        width = std::max(128, width);
        hint.setWidth(width);
    }

    return hint;
}

void TabBar::mousePressEvent(QMouseEvent* event)
{
    event->ignore();

    // Capture where inside the pressed tab the cursor is, so the drag
    // clamp in `mouseMoveEvent` can tell when the tab has been dragged
    // past the first / last position.
    auto const pressed_rect = tabRect(tabAt(event->pos()));
    m_x_position_in_selected_tab_while_dragging = event->pos().x() - pressed_rect.x();

    QTabBar::mousePressEvent(event);
}

void TabBar::mouseMoveEvent(QMouseEvent* event)
{
    event->ignore();

    auto const first_rect = tabRect(0);
    auto const last_rect = tabRect(count() - 1);

    // Valid x range for the cursor: from the first tab's left edge
    // plus the captured grip offset, to the last tab's left edge plus
    // the same offset. Anything outside gets clamped.
    int const min_x = first_rect.x() + m_x_position_in_selected_tab_while_dragging;
    int const max_x = last_rect.x() + m_x_position_in_selected_tab_while_dragging;

    if (event->pos().x() >= min_x && event->pos().x() <= max_x) {
        QTabBar::mouseMoveEvent(event);
        return;
    }

    auto pos = event->pos();
    if (event->pos().x() > max_x) {
        pos.setX(max_x);
    } else if (event->pos().x() < min_x) {
        pos.setX(min_x);
    }
    QMouseEvent synthetic(
        event->type(),
        pos,
        event->globalPosition(),
        event->button(),
        event->buttons(),
        event->modifiers());
    QTabBar::mouseMoveEvent(&synthetic);
}

TabWidget::TabWidget(QWidget* parent)
    : QWidget(parent)
{
    m_tab_bar = new TabBar(this);
    m_tab_bar->setDocumentMode(true);
    m_tab_bar->setElideMode(Qt::TextElideMode::ElideRight);
    m_tab_bar->setMovable(true);
    m_tab_bar->setTabsClosable(true);
    m_tab_bar->setExpanding(false);
    m_tab_bar->setUsesScrollButtons(true);
    m_tab_bar->setDrawBase(false);

    m_stacked_widget = new QStackedWidget(this);

    m_new_tab_button = new QToolButton(this);
    m_new_tab_button->setIconSize(QSize(18, 18));
    m_new_tab_button->setAutoRaise(true);
    m_new_tab_button->setToolTip(QStringLiteral("New Tab"));
    // Ensure the button has enough hit area to be obvious. Tab bars
    // are compact, so without an explicit minimum size the QToolButton
    // collapses to ~20 px and is easy to miss.
    m_new_tab_button->setMinimumSize(QSize(32, 28));
    m_new_tab_button->setIcon(icons::plus(palette().color(QPalette::WindowText)));
    connect(m_new_tab_button, &QToolButton::clicked, this, &TabWidget::new_tab_requested);

    auto* tab_bar_row_layout = new QHBoxLayout();
    tab_bar_row_layout->setSpacing(0);
    tab_bar_row_layout->setContentsMargins(0, 0, 0, 0);
    // Give the tab bar itself the stretch factor so it expands to
    // fill all horizontal space in the row. Without this the bar
    // stays at its cramped initial sizeHint and QTabBar's scroll
    // buttons overlap the visible tab when the widget is narrower
    // than one `tabSizeHint` width. The new-tab button sits
    // immediately to the right of the last tab.
    tab_bar_row_layout->addWidget(m_tab_bar, /*stretch=*/1);
    tab_bar_row_layout->addWidget(m_new_tab_button);

    m_tab_bar_row = new QWidget(this);
    m_tab_bar_row->setLayout(tab_bar_row_layout);

    auto* main_layout = new QVBoxLayout(this);
    main_layout->setSpacing(0);
    main_layout->setContentsMargins(0, 0, 0, 0);
    main_layout->addWidget(m_tab_bar_row);
    main_layout->addWidget(m_stacked_widget, /*stretch=*/1);

    // Keep the stacked widget in sync when the user clicks a tab.
    connect(m_tab_bar, &QTabBar::currentChanged, this, [this](int index) {
        if (index >= 0 && index < m_stacked_widget->count()) {
            m_stacked_widget->setCurrentIndex(index);
        }
        emit current_tab_changed(index);
    });

    connect(m_tab_bar, &QTabBar::tabCloseRequested, this, &TabWidget::tab_close_requested);

    // When the user drags a tab to a new position, mirror the move in
    // the stacked widget so page ↔ tab index stays aligned. Block the
    // stacked widget's own signals during the move so we don't emit a
    // spurious `currentChanged`.
    connect(m_tab_bar, &QTabBar::tabMoved, this, [this](int from, int to) {
        m_stacked_widget->blockSignals(true);
        auto* widget = m_stacked_widget->widget(from);
        m_stacked_widget->removeWidget(widget);
        m_stacked_widget->insertWidget(to, widget);
        m_stacked_widget->setCurrentIndex(m_tab_bar->currentIndex());
        m_stacked_widget->blockSignals(false);
    });

    // Spinner animation timer. Starts only while at least one tab
    // is in `m_loading_tabs`, stops as soon as the set is empty, so
    // idle tabs don't keep a timer running for nothing.
    m_spinner_timer = new QTimer(this);
    m_spinner_timer->setInterval(kSpinnerIntervalMs);
    connect(m_spinner_timer, &QTimer::timeout, this, &TabWidget::tick_spinner);
}

Tab* TabWidget::current_tab() const
{
    return qobject_cast<Tab*>(m_stacked_widget->currentWidget());
}

void TabWidget::add_tab(QWidget* widget, QString const& label)
{
    int const stacked_index = m_stacked_widget->addWidget(widget);
    int const bar_index = m_tab_bar->addTab(label);
    // `addWidget` and `addTab` should return matching indices because
    // we never add out of order, but assert-style guard just in case.
    if (stacked_index != bar_index) {
        // Not fatal — the signals hooked up above keep things in sync
        // on every user interaction — but worth noticing in logs.
        // Avoid QDebug here to keep the dependency list short.
    }
    m_tab_bar->setCurrentIndex(bar_index);
    update_tab_layout();

    // Wire the tab's load signals to the spinner bookkeeping. We
    // only do this for `Tab` instances; any other widget added to
    // the tab container (e.g. a settings page) just won't get a
    // spinner.
    if (auto* tab = qobject_cast<Tab*>(widget)) {
        connect(tab, &Tab::tabLoadStarted, this, [this, tab]() {
            m_loading_tabs.insert(tab);
            if (!m_spinner_timer->isActive()) {
                m_spinner_timer->start();
            }
            paint_spinner_on(tab);
        });
        connect(tab, &Tab::tabLoadFinished, this, [this, tab]() {
            m_loading_tabs.remove(tab);
            if (m_loading_tabs.isEmpty()) {
                m_spinner_timer->stop();
            }
            int const idx = m_stacked_widget->indexOf(tab);
            if (idx >= 0) {
                m_tab_bar->setTabIcon(idx, QIcon());
            }
        });
    }
}

void TabWidget::remove_tab(int index)
{
    if (index < 0 || index >= m_tab_bar->count()) {
        return;
    }

    auto* widget = m_stacked_widget->widget(index);
    m_stacked_widget->removeWidget(widget);
    // If the tab was in the loading set, drop it so the spinner
    // timer can stop when the set empties.
    if (auto* tab = qobject_cast<Tab*>(widget)) {
        m_loading_tabs.remove(tab);
        if (m_loading_tabs.isEmpty()) {
            m_spinner_timer->stop();
        }
    }
    if (widget != nullptr) {
        widget->deleteLater();
    }

    m_tab_bar->removeTab(index);

    if (m_tab_bar->count() > 0 && m_tab_bar->currentIndex() >= 0) {
        m_stacked_widget->setCurrentIndex(m_tab_bar->currentIndex());
    }

    update_tab_layout();
}

bool TabWidget::event(QEvent* event)
{
    // Middle-click on a tab requests close. We handle this at the
    // container level instead of in `TabBar` so the signal routes
    // through the same path as the (x) button close.
    if (event->type() == QEvent::MouseButtonRelease) {
        auto const* mouse_event = static_cast<QMouseEvent const*>(event);
        if (mouse_event->button() == Qt::MiddleButton) {
            if (auto const index = m_tab_bar->tabAt(mouse_event->pos()); index != -1) {
                emit tab_close_requested(index);
                return true;
            }
        }
    }
    return QWidget::event(event);
}

void TabWidget::resizeEvent(QResizeEvent* event)
{
    QWidget::resizeEvent(event);
    update_tab_layout();
}

void TabWidget::update_tab_layout()
{
    int const button_width = m_new_tab_button->sizeHint().width();
    int const available_for_tabs = width() - button_width;
    m_tab_bar->set_available_width(available_for_tabs);
}

void TabWidget::tick_spinner()
{
    // Wrap at 360 so the angle value stays small; cosmetically it
    // doesn't matter where it wraps.
    m_spinner_angle = (m_spinner_angle + kSpinnerStepDegrees) % 360;
    for (Tab* tab : m_loading_tabs) {
        paint_spinner_on(tab);
    }
}

void TabWidget::paint_spinner_on(Tab* tab)
{
    int const idx = m_stacked_widget->indexOf(tab);
    if (idx < 0) {
        return;
    }
    auto const color = palette().color(QPalette::WindowText);
    m_tab_bar->setTabIcon(idx, icons::spinner(color, m_spinner_angle));
}

}
