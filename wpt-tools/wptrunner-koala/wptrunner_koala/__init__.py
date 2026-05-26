"""wptrunner plugin for the koala browser.

Registered via the ``wptrunner.products`` entry-point group in
``pyproject.toml``. After ``pip install -e wpt-tools/wptrunner-koala``,
``wpt run --product=koala`` discovers this package and drives koala
through the subprocess JSON-lines protocol.

See ``project-memory/wpt-integration-spec.md`` Phase 1 for the
architecture.
"""


def load():
    """Return the wptrunner ``Product`` description for koala.

    Called once by wptrunner during product discovery. As a side
    effect, also patches ``tools.wpt.run.check_environ`` so the
    ``/etc/hosts`` check does not block ``wpt run --product=koala``
    — koala consumes WPT's hosts file via ``koala-cli --hosts-file``
    instead of relying on system DNS overrides.
    """
    _install_hosts_check_bypass()

    from wptrunner.products import Product

    from . import koala

    return Product._from_dunder_wptrunner(koala)


def _install_hosts_check_bypass():
    """Add ``"koala"`` to ``tools.wpt.run.check_environ``'s skip set.

    Upstream's ``check_environ`` enforces that ``/etc/hosts`` (or the
    Windows equivalent) contains the WPT hostname mappings before any
    non-builtin product is allowed to run. Koala's hosts-file
    handling makes that requirement redundant, but we cannot patch
    the WPT submodule in-place because the submodule must stay bumpable
    against upstream. Monkey-patching the function's literal
    ``builtin_skip`` set is reversible at process scope and survives
    submodule bumps.
    """
    try:
        from tools.wpt import run as wpt_run
    except ImportError:
        # ``tools.wpt`` is not on sys.path during plain ``import
        # wptrunner_koala`` (e.g. when listing entry points). The
        # bypass only needs to be live when ``wpt run`` is invoked,
        # which is exactly when this import succeeds.
        return

    original = wpt_run.check_environ

    def check_environ(product):
        if product == "koala":
            return
        return original(product)

    wpt_run.check_environ = check_environ
