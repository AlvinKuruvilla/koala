# Open the browser GUI, optionally loading a URL or file path.
#   just gui
#   just gui https://example.com
#   just gui res/test.html
gui url="":
    @if [ -z "{{url}}" ]; then \
        cargo run --bin koala; \
    else \
        cargo run --bin koala -- "{{url}}"; \
    fi

# Run the headless CLI, optionally saving a screenshot.
#   just cli https://example.com
#   just cli res/test.html
#   just cli https://example.com screenshot.png
cli url screenshot="":
    @if [ -z "{{screenshot}}" ]; then \
        cargo run --bin koala-cli -- "{{url}}"; \
    else \
        cargo run --bin koala-cli -- -S "{{screenshot}}" "{{url}}"; \
    fi
