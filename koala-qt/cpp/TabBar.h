// TabBar — custom `QTabBar` with browser-style sizing and drag clamping,
// plus the `TabWidget` container that hosts it alongside a stacked
// content area and a new-tab button.
//
// Modeled on Ladybird's `UI/Qt/TabBar.h`. The port keeps:
//
// - Per-tab min/max width (128..225 px) computed from available width,
//   so tabs shrink uniformly instead of overflowing.
// - Drag-to-reorder with a movement clamp that prevents tabs from
//   escaping their row horizontally.
// - Middle-click on a tab emits `tab_close_requested`.
// - A tab-bar row composed of the bar, a `QToolButton` new-tab button,
//   and a stretch spacer, with the content stacked below.
//
// And drops (for now):
//
// - TVG theme-aware icons (Ladybird's LibGfx icon engine) — the new-tab
//   button uses a standard Qt pixmap icon until koala has its own set.
// - Per-tab context menu (will land when `BrowserWindow` grows menu
//   actions for Back/Forward/Reload-in-tab etc.).
// - `TabBarButton` — a small flat push button used by Ladybird for
//   per-tab audio indicators; not needed until we wire up audio.

#pragma once

#include <QPointer>
#include <QSize>
#include <QStackedWidget>
#include <QTabBar>
#include <QToolButton>
#include <QWidget>

class QEvent;
class QMouseEvent;
class QResizeEvent;

namespace koala {

class Tab;
class TabWidget;

class TabBar : public QTabBar {
    Q_OBJECT

public:
    explicit TabBar(TabWidget* parent);
    ~TabBar() override = default;

    // Tells the tab bar how much horizontal space it has to work with.
    // Set by the owning `TabWidget` on resize so `tabSizeHint` can
    // divide it across tabs.
    void set_available_width(int width);

    QSize tabSizeHint(int index) const override;

protected:
    void mousePressEvent(QMouseEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;

private:
    QPointer<TabWidget> m_tab_widget;

    int m_available_width { 0 };
    // X offset of the cursor relative to the start of the tab being
    // dragged. Captured on mouse press and used by `mouseMoveEvent` to
    // keep the drag clamped inside the valid row.
    int m_x_position_in_selected_tab_while_dragging { 0 };
};

class TabWidget : public QWidget {
    Q_OBJECT

public:
    explicit TabWidget(QWidget* parent = nullptr);
    ~TabWidget() override = default;

    TabBar* tab_bar() const { return m_tab_bar; }

    // Adds `widget` as a new tab with the given label. The tab becomes
    // the current tab. Ownership of `widget` passes to the internal
    // `QStackedWidget`.
    void add_tab(QWidget* widget, QString const& label);

    // Removes the tab at `index` and deletes its page widget.
    void remove_tab(int index);

    int count() const { return m_tab_bar->count(); }
    int current_index() const { return m_tab_bar->currentIndex(); }
    void set_current_index(int index) { m_tab_bar->setCurrentIndex(index); }

    QWidget* tab(int index) const { return m_stacked_widget->widget(index); }

    // The currently active `Tab` (down-cast from the stacked widget's
    // current page). Returns nullptr when there are no tabs or when
    // the current page is not a `Tab` instance.
    Tab* current_tab() const;

    void set_tab_text(int index, QString const& text) { m_tab_bar->setTabText(index, text); }

signals:
    void current_tab_changed(int index);
    void tab_close_requested(int index);
    void new_tab_requested();

protected:
    bool event(QEvent* event) override;
    void resizeEvent(QResizeEvent* event) override;

private:
    // Recomputes the horizontal space available to the tab bar itself
    // (window width minus the new-tab button) and pushes it into
    // `TabBar::set_available_width`.
    void update_tab_layout();

    TabBar* m_tab_bar { nullptr };
    QStackedWidget* m_stacked_widget { nullptr };
    QToolButton* m_new_tab_button { nullptr };
    QWidget* m_tab_bar_row { nullptr };
};

}
