"""Read-only bridge to the established legacy decoders. Writes JSON to stdout."""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "backend"))
import server

def main():
    snapshot = '--stdin' in sys.argv[2:]
    path = Path(sys.argv[1]).resolve(strict=not snapshot)
    if not snapshot and path.stat().st_size > 128 * 1024 * 1024:
        raise ValueError("Legacy source exceeds 128 MiB")
    data = sys.stdin.buffer.read(128 * 1024 * 1024 + 1) if snapshot else path.read_bytes()
    if len(data) > 128 * 1024 * 1024:
        raise ValueError("Legacy source exceeds 128 MiB")
    store = server.ProjectStore(path.parent)
    result = store.import_bytes(data, path.name, as_json=False)
    sys.stdout.buffer.write(json.dumps(result["document"], ensure_ascii=False, allow_nan=False, separators=(",", ":")).encode("utf-8"))

if __name__ == "__main__":
    main()
