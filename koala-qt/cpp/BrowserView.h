// BrowserView — custom QWidget that hosts a koala-browser rendered page.
//
// The widget owns a `rust::Box<BrowserPage>` (created via the
// `new_browser_page` free function from the cxx bridge) and a
// `QImage` caching the most recent rasterisation. When the widget
// resizes or its content changes we ask the engine to re-render at
// the current physical pixel dimensions, copy the RGBA buffer into
// a `QImage` tagged with the widget's `devicePixelRatio`, and call
// `update()` to schedule a paint.
//
// The rasterisation runs on the Qt GUI thread for now. That's fine
// while the only content is the static landing page, but will need
// to move to a worker thread before we wire up real URL navigation —
// layout + paint + rasterise on a large document will visibly hitch
// the event loop.

#pragma once

#include "koala-qt/src/bridge.rs.h"

#include <QImage>
#include <QWidget>

class QPaintEvent;
class QResizeEvent;

namespace koala {

class BrowserView : public QWidget {
    Q_OBJECT

public:
    explicit BrowserView(QWidget* parent = nullptr);
    ~BrowserView() override = default;

    // Replace the page's content with the built-in landing page and
    // trigger a re-render. Called from `Tab` when a fresh tab opens
    // and whenever the user hits the Home action.
    void load_landing_page();

    // Replace the page's content with raw HTML and re-render.
    // Intended for the future `navigate_to_url_bar_text` path.
    void load_html(QString const& html);

    // Re-runs the rendering pipeline at the current widget size
    // without touching the page content. Wired to the Tab toolbar's
    // Reload action.
    void reload_current();

protected:
    void paintEvent(QPaintEvent* event) override;
    void resizeEvent(QResizeEvent* event) override;

private:
    // Runs the engine pipeline at the widget's current physical
    // pixel dimensions and replaces `m_image`. Safe to call when
    // the widget has zero area — it no-ops in that case.
    void re_render();

    rust::Box<BrowserPage> m_page;
    QImage m_image;
};

}
