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
    // Synchronous — runs the parse on the calling (GUI) thread.
    void load_html(QString const& html);

    // Queue an async URL load on the loader worker thread. Returns
    // immediately; the new page appears in the viewport once the
    // loader worker finishes fetching and parsing, which is picked
    // up by `poll_render_result`.
    void load_url(QString const& url);

    // Re-fetch the most recently navigated URL (async, via the
    // loader worker). No-op when the current page is the landing
    // page or any other in-memory HTML. Wired to the Tab toolbar's
    // Reload action.
    void reload_current();

    // Navigate one step back / forward in the per-tab history
    // stack. Returns immediately; the actual content appears once
    // the loader worker finishes the re-fetch. Wired to the Tab
    // toolbar's Back / Forward actions.
    void go_back();
    void go_forward();

    // Queries used by `Tab` to enable/disable the toolbar
    // Back/Forward actions after every load. Both forward directly
    // into `BrowserPage`.
    bool can_go_back() const;
    bool can_go_forward() const;

signals:
    // Emitted once when the view hands a load request off to the
    // loader worker (either via `load_url` or a successful
    // `reload_current` re-fetch). `Tab` listens for this to show
    // its progress indicator.
    void loadStarted();

    // Emitted once per `loadStarted` when the loader worker
    // finishes the request, whether the load succeeded or failed.
    // `Tab` listens for this to hide its progress indicator.
    void loadFinished();

    // Emitted after a successful load (either async via the
    // loader worker or synchronous via `load_html`) with the
    // new document's `<title>` text. Empty string when the
    // document has no `<title>` element. `TabWidget` listens
    // for this to update the tab bar label.
    void titleChanged(QString const& title);

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
