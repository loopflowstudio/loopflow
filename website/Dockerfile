FROM python:3.12-slim

WORKDIR /app

COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /usr/local/bin/

COPY pyproject.toml .
RUN uv pip install --system --no-cache .

COPY . .

EXPOSE 5001

CMD ["sh", "-c", "uvicorn main:app --host 0.0.0.0 --port ${PORT:-5001}"]
