// cxx bridge entry point for koala-qt.
//
// This file is deliberately thin: it owns the `QApplication`, hands
// argv through to it, instantiates the top-level `BrowserWindow`, and
// runs the event loop. Everything else lives in `BrowserWindow` and
// its friends.

#include "koala_window.h"

#include "BrowserWindow.h"

#include <QApplication>

#include <string>
#include <vector>

namespace koala {

std::int32_t run_event_loop(rust::Vec<rust::String> argv)
{
    // QApplication mutates argc/argv in place while parsing Qt's own
    // command-line flags, so we copy the strings into a stable buffer
    // that outlives the QApplication construction. `rust::String` is
    // not guaranteed to be NUL-terminated, which is why we materialise
    // each one into a `std::string` first.
    std::vector<std::string> storage;
    storage.reserve(argv.size());
    for (auto const& s : argv) {
        storage.emplace_back(std::string(s));
    }

    std::vector<char*> ptrs;
    ptrs.reserve(storage.size() + 1);
    for (auto& s : storage) {
        ptrs.push_back(s.data());
    }
    ptrs.push_back(nullptr);

    int argc = static_cast<int>(storage.size());
    QApplication app(argc, ptrs.data());

    BrowserWindow window;
    window.show();
    return app.exec();
}

}
