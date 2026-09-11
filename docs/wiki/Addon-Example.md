# Build your first identifier

This deliberately small Python addon reports the recording size and supplied tuning metadata. It demonstrates the extension contract without claiming to recognize a protocol. See [[Addon-Development]] for the full API and [[Protocol-Catalog]] for production examples.

Create a new directory under `~/.config/thugsrf/identifiers/file-facts/` (or the equivalent XDG configuration root).

Save this as `addon.toml`:

```toml
api_version = 1
name = "file-facts"
kind = "identifiers"
description = "Report basic file and tuning facts; no protocol identification"
command = ["python3", "identify.py"]
enabled = false
timeout_seconds = 10
formats = ["cs8", "cu8", "wav"]
requires = ["python3"]
```

Save this as `identify.py` in the same directory:

```python
import json
import pathlib
import sys

request = json.load(sys.stdin)
if request.get("api_version") != 1:
    raise ValueError("unsupported API version")
source = request["input"]
path = pathlib.Path(source["path"])
result = {
    "api_version": 1,
    "identifier": "file-facts",
    "events": [],
    "facts": {
        "bytes": path.stat().st_size,
        "format": source["format"],
        "center_hz": source["center_hz"],
        "sample_rate": source["sample_rate"],
    },
    "notes": ["File metadata only; no modulation or protocol was identified."],
}
print(json.dumps(result))
```

Enable and run it against an existing recording:

```sh
thugsrf addon list
thugsrf addon enable file-facts
thugsrf addon run file-facts signal.cs8
thugsrf identify signal.cs8
```

The host launches the process from its addon directory and supplies one JSON request on stdin. Emit exactly one JSON object on stdout. Use stderr for debugging during direct development, but remember that the host suppresses addon stderr. The result is saved as a finding.

To use Rust or Go, compile a Linux executable into the module directory and change `command` to `["./my-identifier"]`. Keep the same JSON interface. Bound memory, samples and execution time inside the addon; host deadlines are not a sandbox. Preserve source provenance and distinguish a candidate from a decoded packet.
