import ollama

EMBED_PREFIX = "search_query: "


def list_models() -> list[str]:
    try:
        return [m.model for m in ollama.list().models]
    except Exception:
        return []


def embed(text: str, model: str) -> list[float]:
    prefixed = f"{EMBED_PREFIX}{text}"
    result = ollama.embed(model=model, input=[prefixed])
    return result.embeddings[0]
