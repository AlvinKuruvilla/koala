// Icons — programmatically-drawn browser chrome icons.
//
// Ladybird ships a bundled set of TVG icons tinted against the current
// palette. Koala has no icon set yet, and the Qt standard pixmap icons
// (`QStyle::SP_ArrowBack` and friends) look coarse and unstylish on
// macOS. As an interim solution we render a small set of simple,
// high-quality icons with `QPainter` at load time and cache them in
// a `QIcon`.
//
// Every icon is drawn from vector primitives (`drawLine`, `drawArc`)
// with antialiasing enabled, and takes a `QColor` argument so the
// caller can pass the current text colour from the widget palette.
// This means the icons pick up the user's light/dark theme
// automatically.

#pragma once

#include <QColor>
#include <QIcon>

namespace koala::icons {

// All icons are rendered at 32 px on the native device pixel ratio and
// scaled down by the QToolBar to its icon size. 32 is enough headroom
// that HiDPI displays still look crisp.
constexpr int kRasterSize = 32;

// Left-pointing chevron (Back action).
QIcon back(QColor const& color);

// Right-pointing chevron (Forward action).
QIcon forward(QColor const& color);

// Circular arrow (Reload action).
QIcon reload(QColor const& color);

// Simple house silhouette (Home action).
QIcon home(QColor const& color);

// Plus sign (New Tab button).
QIcon plus(QColor const& color);

}
