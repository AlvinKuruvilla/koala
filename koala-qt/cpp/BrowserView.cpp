// BrowserView implementation.
//
// Pixel format: koala-browser writes RGBA with straight (non-premultiplied)
// alpha, matching `QImage::Format_RGBA8888`. Qt's own painter prefers
// premultiplied for blending performance, but at these sizes the
// difference is invisible and keeping the format identical avoids a
// per-scanline conversion.
//
// Scanline copy: we could construct a non-owning `QImage` that
// references the `rust::Vec<uint8_t>` bytes directly, but then the
// Vec would need to outlive the QImage (which includes spans where
// Qt keeps an internal copy for DPR scaling). A one-time memcpy into
// a self-owning QImage is cheap and removes the lifetime coupling.

#include "BrowserView.h"

#include <QImage>
#include <QPaintEvent>
#include <QPainter>
#include <QResizeEvent>

#include <algorithm>
#include <cmath>
#include <cstring>

namespace koala {

BrowserView::BrowserView(QWidget* parent)
    : QWidget(parent)
    , m_page(new_browser_page())
{
    // A background colour lands underneath the rendered content
    // during the brief window before the first render completes.
    setAutoFillBackground(true);
    auto pal = palette();
    pal.setColor(QPalette::Window, Qt::white);
    setPalette(pal);
}

void BrowserView::load_landing_page()
{
    m_page->load_landing_page();
    re_render();
}

void BrowserView::load_html(QString const& html)
{
    auto const utf8 = html.toUtf8();
    m_page->load_html(rust::Str(utf8.constData(), static_cast<size_t>(utf8.size())));
    re_render();
}

void BrowserView::reload_current()
{
    re_render();
}

void BrowserView::paintEvent(QPaintEvent* /*event*/)
{
    QPainter painter(this);
    if (m_image.isNull()) {
        painter.fillRect(rect(), Qt::white);
        return;
    }
    painter.drawImage(QPoint(0, 0), m_image);
}

void BrowserView::resizeEvent(QResizeEvent* event)
{
    QWidget::resizeEvent(event);
    re_render();
}

void BrowserView::re_render()
{
    // Bail out cheaply for degenerate sizes. Qt calls `resizeEvent`
    // with zero-sized geometries during tab creation and teardown.
    if (width() <= 0 || height() <= 0) {
        m_image = QImage();
        update();
        return;
    }

    // Render at physical pixels so HiDPI displays get crisp output.
    // We tag the resulting `QImage` with the same ratio so Qt draws
    // it at the logical widget size.
    qreal const dpr = devicePixelRatioF();
    int const physical_w = std::max(1, static_cast<int>(std::round(width() * dpr)));
    int const physical_h = std::max(1, static_cast<int>(std::round(height() * dpr)));

    rust::Vec<std::uint8_t> pixels = m_page->render_to_rgba(
        static_cast<std::uint32_t>(physical_w),
        static_cast<std::uint32_t>(physical_h));

    auto const expected = static_cast<std::size_t>(physical_w)
        * static_cast<std::size_t>(physical_h) * 4;
    if (pixels.size() != expected) {
        // Pipeline returned a buffer that doesn't match the requested
        // size — bail rather than paint garbage. This only happens if
        // the Rust side hit an error path we haven't surfaced yet.
        m_image = QImage();
        update();
        return;
    }

    QImage image(physical_w, physical_h, QImage::Format_RGBA8888);
    // `QImage::scanLine` stride (`bytesPerLine`) may be >4*w on some
    // platforms due to alignment; copy row by row to be safe even
    // though the current Qt builds use exactly 4*w.
    std::size_t const src_stride = static_cast<std::size_t>(physical_w) * 4;
    for (int y = 0; y < physical_h; ++y) {
        auto* dst = image.scanLine(y);
        std::memcpy(dst, pixels.data() + static_cast<std::size_t>(y) * src_stride, src_stride);
    }
    image.setDevicePixelRatio(dpr);
    m_image = std::move(image);
    update();
}

}
