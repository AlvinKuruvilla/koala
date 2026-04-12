// koala-qt — Qt-based browser UI for koala.
//
// The Rust entry point is deliberately thin: collect process arguments,
// hand them to the C++ widget layer, and return whatever exit code Qt's
// event loop produced. Everything user-facing — windows, tabs, URL bar,
// menus — is built in C++ out of real QWidgets, mirroring Ladybird's
// `UI/Qt/` structure.

mod bridge;
mod browser_page;
mod landing;

fn main() -> std::process::ExitCode {
    // The C++ shim reconstructs a stable `char**` buffer from these
    // strings before handing them to QApplication (which mutates argc/argv
    // in place while parsing its own command-line flags).
    let args: Vec<String> = std::env::args().collect();
    let exit_code = bridge::ffi::run_event_loop(args);
    match u8::try_from(exit_code) {
        Ok(code) => std::process::ExitCode::from(code),
        Err(_) => std::process::ExitCode::FAILURE,
    }
}
