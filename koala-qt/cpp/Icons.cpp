// Programmatically-drawn browser icons.
//
// Each helper allocates a 32×32 transparent `QPixmap`, paints the shape
// in the caller-supplied colour, and wraps it in a `QIcon`. 32 px gives
// enough headroom for crisp rendering on 2x HiDPI displays when the
// toolbar down-samples to 16 px.
//
// All strokes use a 2.5 px pen so thin features stay visible after
// downscaling without turning the icons into blobs at 16 px. Rounded
// caps and joins avoid jagged ends.

#include "Icons.h"

#include <QIcon>
#include <QPainter>
#include <QPainterPath>
#include <QPen>
#include <QPixmap>
#include <QRectF>

namespace koala::icons {

namespace {

// Small helper to set up a painter with the conventions every icon
// wants: antialiasing on, stroke in the caller's colour, 2.5 px pen
// with rounded caps/joins, no fill. Returns the painter configured
// on `pixmap`.
void configure_painter(QPainter& painter, QColor const& color, qreal stroke_width = 2.5)
{
    painter.setRenderHint(QPainter::Antialiasing, true);
    QPen pen(color);
    pen.setWidthF(stroke_width);
    pen.setCapStyle(Qt::RoundCap);
    pen.setJoinStyle(Qt::RoundJoin);
    painter.setPen(pen);
    painter.setBrush(Qt::NoBrush);
}

// Allocates a 32×32 transparent pixmap ready for drawing.
QPixmap blank_pixmap()
{
    QPixmap pixmap(kRasterSize, kRasterSize);
    pixmap.fill(Qt::transparent);
    return pixmap;
}

} // namespace

QIcon back(QColor const& color)
{
    // Chevron pointing left: two lines meeting at the midpoint on the
    // left third of the canvas. Centred vertically.
    auto pixmap = blank_pixmap();
    QPainter p(&pixmap);
    configure_painter(p, color);

    qreal const cx = kRasterSize * 0.42;
    qreal const top = kRasterSize * 0.28;
    qreal const bottom = kRasterSize * 0.72;
    qreal const right = kRasterSize * 0.62;

    QPainterPath path;
    path.moveTo(right, top);
    path.lineTo(cx, kRasterSize / 2.0);
    path.lineTo(right, bottom);
    p.drawPath(path);

    return QIcon(pixmap);
}

QIcon forward(QColor const& color)
{
    // Mirror of `back`: chevron pointing right.
    auto pixmap = blank_pixmap();
    QPainter p(&pixmap);
    configure_painter(p, color);

    qreal const cx = kRasterSize * 0.58;
    qreal const top = kRasterSize * 0.28;
    qreal const bottom = kRasterSize * 0.72;
    qreal const left = kRasterSize * 0.38;

    QPainterPath path;
    path.moveTo(left, top);
    path.lineTo(cx, kRasterSize / 2.0);
    path.lineTo(left, bottom);
    p.drawPath(path);

    return QIcon(pixmap);
}

QIcon reload(QColor const& color)
{
    // Circular arrow: ~270° arc with a small arrowhead at one end.
    // The break is at the top of the circle (12 o'clock position).
    auto pixmap = blank_pixmap();
    QPainter p(&pixmap);
    configure_painter(p, color);

    qreal const pad = kRasterSize * 0.22;
    QRectF const rect(pad, pad, kRasterSize - 2 * pad, kRasterSize - 2 * pad);

    // `drawArc` takes angles in sixteenths of a degree. Start at
    // 80° (just past the top, slightly right), sweep 270° clockwise.
    int const start_angle = 80 * 16;
    int const sweep = -270 * 16;
    p.drawArc(rect, start_angle, sweep);

    // Arrowhead at the end of the sweep (top of circle, slightly left).
    // Two short lines forming a >-shape rotated to point along the
    // tangent at that point.
    qreal const tip_x = rect.center().x() + 1.5;
    qreal const tip_y = rect.top() + 1.0;
    qreal const arrow = kRasterSize * 0.14;

    QPainterPath head;
    head.moveTo(tip_x - arrow, tip_y - arrow * 0.2);
    head.lineTo(tip_x, tip_y);
    head.lineTo(tip_x - arrow * 0.2, tip_y + arrow);
    p.drawPath(head);

    return QIcon(pixmap);
}

QIcon home(QColor const& color)
{
    // Simple house: triangular roof on top of a square body. Doorway
    // is a small rectangle centred at the bottom.
    auto pixmap = blank_pixmap();
    QPainter p(&pixmap);
    configure_painter(p, color);

    qreal const roof_peak_x = kRasterSize / 2.0;
    qreal const roof_peak_y = kRasterSize * 0.22;
    qreal const eave_y = kRasterSize * 0.48;
    qreal const wall_left = kRasterSize * 0.22;
    qreal const wall_right = kRasterSize * 0.78;
    qreal const floor_y = kRasterSize * 0.78;

    QPainterPath house;
    house.moveTo(wall_left, eave_y);
    house.lineTo(roof_peak_x, roof_peak_y);
    house.lineTo(wall_right, eave_y);
    house.lineTo(wall_right, floor_y);
    house.lineTo(wall_left, floor_y);
    house.closeSubpath();

    // Doorway cut-out (drawn as a separate subpath on top).
    qreal const door_half = kRasterSize * 0.09;
    qreal const door_top = kRasterSize * 0.58;
    house.moveTo(roof_peak_x - door_half, floor_y);
    house.lineTo(roof_peak_x - door_half, door_top);
    house.lineTo(roof_peak_x + door_half, door_top);
    house.lineTo(roof_peak_x + door_half, floor_y);

    p.drawPath(house);

    return QIcon(pixmap);
}

QIcon plus(QColor const& color)
{
    // Bold + sign for the New Tab button. Thicker stroke than the
    // navigation chevrons so it reads as a distinct control from the
    // rest of the toolbar.
    auto pixmap = blank_pixmap();
    QPainter p(&pixmap);
    configure_painter(p, color, /*stroke_width=*/3.0);

    qreal const half = kRasterSize * 0.3;
    qreal const cx = kRasterSize / 2.0;
    qreal const cy = kRasterSize / 2.0;

    p.drawLine(QPointF(cx - half, cy), QPointF(cx + half, cy));
    p.drawLine(QPointF(cx, cy - half), QPointF(cx, cy + half));

    return QIcon(pixmap);
}

QIcon spinner(QColor const& color, int angle_degrees)
{
    // 270° arc inside the same inset rectangle the `reload` icon
    // uses, minus the arrowhead. `drawArc` takes 1/16° units; a
    // negative span sweeps clockwise from the start angle. The
    // caller advances `angle_degrees` on a timer to animate the
    // rotation.
    auto pixmap = blank_pixmap();
    QPainter p(&pixmap);
    configure_painter(p, color, /*stroke_width=*/3.0);

    qreal const pad = kRasterSize * 0.22;
    QRectF const rect(pad, pad, kRasterSize - 2 * pad, kRasterSize - 2 * pad);

    int const start_angle = angle_degrees * 16;
    int const sweep = -270 * 16;
    p.drawArc(rect, start_angle, sweep);

    return QIcon(pixmap);
}

}
