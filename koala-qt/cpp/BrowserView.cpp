// BrowserView implementation.
//
// Pixel format: koala-browser writes RGBA with straight (non-premultiplied)
// alpha, matching `QImage::Format_RGBA8888`. Qt's own painter prefers
// premultiplied for blending performance, but at these sizes the
// difference is invisible and keeping the format identical avoids a
// per-scanline conversion.
//
// Threading: the render pipeline runs on a dedicated Rust worker
// thread inside `BrowserPage`. This widget only sees it through two
// non-blocking cxx calls (`request_render` and `try_take_render_result`).
// Frame delivery uses a `QTimer` polling at ~60 Hz rather than an
// event-driven callback because that keeps the bridge surface small
// and avoids marshalling callbacks across the Rust/C++ boundary.

#include "BrowserView.h"

#include <QImage>
#include <QPaintEvent>
#include <QPainter>
#include <QResizeEvent>
#include <QTimer>

#include <algorithm>
#include <cmath>
#include <cstring>

namespace koala {

namespace {
// ~60 Hz poll. Adds at most ~16 ms between a frame finishing on the
// worker and reaching the screen. For a ~180 ms rasterize that's
// imperceptible; for much faster rasterizes (future GPU backend) we
// should switch to event-driven delivery.
constexpr int kPollIntervalMs = 16;
}

BrowserView::BrowserView(QWidget* parent)
    : QWidget(parent)
    , m_page(new_browser_page())
{
    setAutoFillBackground(true);
    auto pal = palette();
    pal.setColor(QPalette::Window, Qt::white);
    setPalette(pal);

    m_poll_timer = new QTimer(this);
    m_poll_timer->setInterval(kPollIntervalMs);
    connect(m_poll_timer, &QTimer::timeout, this, &BrowserView::poll_render_result);
    m_poll_timer->start();
}

void BrowserView::load_landing_page()
{
    m_page->load_landing_page();
    request_render();
}

void BrowserView::load_html(QString const& html)
{
    auto const utf8 = html.toUtf8();
    m_page->load_html(rust::Str(utf8.constData(), static_cast<std::size_t>(utf8.size())));
    // Synchronous load: the new title is available immediately,
    // no worker round trip needed.
    {
        rust::String title = m_page->current_title();
        emit titleChanged(QString::fromUtf8(title.data(), static_cast<int>(title.size())));
    }
    request_render();
}

void BrowserView::load_url(QString const& url)
{
    auto const utf8 = url.toUtf8();
    m_page->request_load(rust::Str(utf8.constData(), static_cast<std::size_t>(utf8.size())));
    emit loadStarted();
    // The loader worker runs asynchronously; `poll_render_result`
    // will pick up the new page state and trigger a render (and
    // emit `loadFinished`) once the fetch + parse completes.
}

void BrowserView::reload_current()
{
    // Re-fetch the current URL via the loader worker when possible.
    // For in-memory documents (landing page) `reload_current_url`
    // returns false and we just re-render the existing state so
    // Reload still has a visible effect.
    if (m_page->reload_current_url()) {
        emit loadStarted();
    } else {
        request_render();
    }
}

void BrowserView::go_back()
{
    if (m_page->go_back()) {
        emit loadStarted();
    }
}

void BrowserView::go_forward()
{
    if (m_page->go_forward()) {
        emit loadStarted();
    }
}

bool BrowserView::can_go_back() const
{
    return m_page->can_go_back();
}

bool BrowserView::can_go_forward() const
{
    return m_page->can_go_forward();
}

void BrowserView::paintEvent(QPaintEvent* /*event*/)
{
    QPainter painter(this);
    // Fill the whole widget with the background first so any area
    // the cached frame doesn't cover (the new strip revealed during
    // a resize-larger, for example) shows cleanly instead of
    // garbage.
    painter.fillRect(rect(), Qt::white);
    if (m_image.isNull()) {
        return;
    }
    // Draw the cached frame at its natural size (no stretch). During
    // an active resize the frame may not fill the widget — that's
    // fine, the debounced `m_resize_debounce` timer posts a fresh
    // render shortly after the drag pauses.
    painter.drawImage(QPoint(0, 0), m_image);
}

void BrowserView::resizeEvent(QResizeEvent* event)
{
    QWidget::resizeEvent(event);
    // Post a render on every resize event. The worker coalesces
    // queued jobs so intermediate sizes never actually hit the
    // rasterizer — we end up with one render per worker cycle
    // (~180 ms for the landing page), which is slow but gives
    // the user live content updates during a drag instead of a
    // frozen frame. The real fix is a faster rasterizer; until
    // then this is the least-bad trade-off.
    request_render();
}

void BrowserView::request_render()
{
    if (width() <= 0 || height() <= 0) {
        return;
    }

    qreal const dpr = devicePixelRatioF();
    int const physical_w = std::max(1, static_cast<int>(std::round(width() * dpr)));
    int const physical_h = std::max(1, static_cast<int>(std::round(height() * dpr)));

    m_page->request_render(
        static_cast<std::uint32_t>(physical_w),
        static_cast<std::uint32_t>(physical_h));
}

void BrowserView::poll_render_result()
{
    // First drain the loader worker. A completed load arrives as
    // either `state_swapped=true` (new page ready — trigger a
    // render) or `state_swapped=false, load_finished=true` (error
    // case). Either way, `load_finished` lets us toggle off any
    // loading indicator the Tab is showing.
    auto const load_update = m_page->try_take_load_result();
    if (load_update.state_swapped) {
        request_render();
        {
        rust::String title = m_page->current_title();
        emit titleChanged(QString::fromUtf8(title.data(), static_cast<int>(title.size())));
    }
    }
    if (load_update.load_finished) {
        emit loadFinished();
    }

    auto const result = m_page->try_take_render_result();
    if (result.pixels.size() == 0) {
        return;
    }

    auto const w = static_cast<int>(result.width);
    auto const h = static_cast<int>(result.height);
    auto const expected = static_cast<std::size_t>(w) * static_cast<std::size_t>(h) * 4;
    if (result.pixels.size() != expected) {
        // Malformed frame — skip rather than paint garbage.
        return;
    }

    QImage image(w, h, QImage::Format_RGBA8888);
    std::size_t const src_stride = static_cast<std::size_t>(w) * 4;
    for (int y = 0; y < h; ++y) {
        auto* dst = image.scanLine(y);
        std::memcpy(
            dst,
            result.pixels.data() + static_cast<std::size_t>(y) * src_stride,
            src_stride);
    }
    image.setDevicePixelRatio(devicePixelRatioF());
    m_image = std::move(image);
    update();
}

}
