// BrowserView — custom QWidget that hosts a koala-browser rendered page.
//
// The widget owns a `rust::Box<BrowserPage>` (created via the
// `new_browser_page` free function from the cxx bridge) and a
// `QImage` caching the most recent rasterisation. `BrowserPage`
// spawns a Rust worker thread internally; every `request_render`
// call posts a job to that thread and returns immediately, so the
// Qt event loop never blocks on the rasterizer. Finished frames
// are picked up by `poll_render_result`, which a `QTimer` fires at
// ~60 Hz, and painted in the widget's `paintEvent` like any other
// `QImage`.

#pragma once

#include "koala-qt/src/bridge.rs.h"

#include <QImage>
#include <QWidget>

class QPaintEvent;
class QResizeEvent;
class QTimer;

namespace koala {

class BrowserView : public QWidget {
    Q_OBJECT

public:
    explicit BrowserView(QWidget* parent = nullptr);
    ~BrowserView() override = default;

    // Replace the page's content with the built-in landing page and
    // queue a render at the current widget size. Called from `Tab`
    // when a fresh tab opens and whenever the user hits Home.
    void load_landing_page();

    // Replace the page's content with raw HTML and queue a render.
    void load_html(QString const& html);

    // Queue a fresh render of the current content at the current
    // widget size. Wired to the Tab toolbar's Reload action.
    void reload_current();

protected:
    void paintEvent(QPaintEvent* event) override;
    void resizeEvent(QResizeEvent* event) override;

private:
    // Posts a render job to the Rust worker thread at the widget's
    // current physical pixel dimensions. No-ops for zero-sized
    // widgets (Qt hands us those during tab creation/teardown).
    void request_render();

    // Polled by `m_poll_timer`. Checks the worker for a finished
    // frame; if one is available, copies its bytes into a `QImage`,
    // tags it with the widget's `devicePixelRatio`, and schedules a
    // paint.
    void poll_render_result();

    rust::Box<BrowserPage> m_page;
    QImage m_image;
    QTimer* m_poll_timer { nullptr };
};

}
