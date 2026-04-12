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
    request_render();
}

void BrowserView::reload_current()
{
    request_render();
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
