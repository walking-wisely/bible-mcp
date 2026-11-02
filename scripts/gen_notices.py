import json
import subprocess

HEADER = """# Third-party notices

Bible MCP bundles or depends on the following.

## Data

- **World English Bible (WEB)** text: public domain.
- **Cross-references** from OpenBible.info (https://www.openbible.info/labs/cross-references/),
  licensed CC-BY 4.0 (https://creativecommons.org/licenses/by/4.0/). Votes are used to rank
  and filter the references.

## Model

- **nomic-embed-text-v1.5** (https://huggingface.co/nomic-ai/nomic-embed-text-v1.5) by Nomic AI,
  Apache License 2.0. The stored verse vectors were produced with it, and queries are embedded
  with it locally or through a hosted provider.

## Bundled runtime

- **ONNX Runtime** (https://github.com/microsoft/onnxruntime), MIT License,
  Copyright (c) Microsoft Corporation. Release archives ship its shared library and its own
  license files next to the `bible-mcp` executable.

## Rust dependencies

Each crate is distributed under the license shown; full license texts are in the crate sources.

| Crate | Version | License |
| --- | --- | --- |
"""


def main():
    meta = json.loads(
        subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--locked"])
    )
    root = {p["id"] for p in meta["packages"] if p["name"] == "bible-mcp"}
    rows = sorted(
        (p["name"], p["version"], p["license"] or "see crate")
        for p in meta["packages"]
        if p["id"] not in root and p["source"]
    )
    out = HEADER + "".join(f"| {n} | {v} | {l} |\n" for n, v, l in rows)
    open("THIRD_PARTY_NOTICES.md", "w").write(out)
    print(len(rows), "crates")


main()
