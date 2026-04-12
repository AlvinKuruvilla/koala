// Public C++ surface exposed to Rust via the cxx bridge.
//
// Only the symbols declared in the `koala` namespace here are visible
// from Rust. Everything else — QMainWindow subclasses, helper widgets,
// Q_OBJECT types — stays in the C++ implementation files and is never
// referenced across the bridge.

#pragma once

#include "rust/cxx.h"

#include <cstdint>

namespace koala {

// Runs a Qt event loop with a single empty `QMainWindow` titled "Koala".
//
// `argv` is an owned `rust::Vec<rust::String>` (Rust's `Vec<String>`).
// The shim copies its contents into a mutable `char**` buffer before
// handing it to `QApplication`, which mutates argc/argv in place while
// parsing its own command-line flags.
std::int32_t run_event_loop(rust::Vec<rust::String> argv);

}
