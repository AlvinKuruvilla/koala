# koala-lab

Measure koala builds against each other and report what changed

## Development

This project uses [uv](https://docs.astral.sh/uv/) for dependencies and
tooling.

```bash
uv sync                    # create .venv and install deps + dev tools
uv run pytest              # run tests
uv run ruff check .        # lint
uv run ruff format .       # format
uv run mypy src            # type-check (strict)
```

