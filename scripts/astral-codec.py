"""Read-only bridge to the established legacy decoders. Writes JSON to stdout."""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "backend"))
import server

def main():
    path = Path(sys.argv[1]).resolve(strict=True)
    if path.stat().st_size > 128 * 1024 * 1024:
        raise ValueError("Legacy source exceeds 128 MiB")
    store = server.ProjectStore(path.parent)
    result = store.import_bytes(path.read_bytes(), path.name, as_json=False)
    sys.stdout.buffer.write(json.dumps(result["document"], ensure_ascii=False, allow_nan=False, separators=(",", ":")).encode("utf-8"))

if __name__ == "__main__":
    main()
