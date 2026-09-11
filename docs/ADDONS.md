# Addon API v1

A module is a directory in the user's `decoders/` or `identifiers/` configuration directory. It contains `addon.toml`:

```toml
api_version = 1
name = "my-decoder"
kind = "decoders"
description = "Describe actual capabilities here"
command = ["python3", "decode.py"]
enabled = false
timeout_seconds = 10
```

The command is executed directly, without a shell, with its working directory set to the module directory. A compiled module can use `command = ["./my-decoder"]`. Names must be unique across both kinds. Timeout is 1–120 seconds. Enabled modules execute only when invoked, not merely when discovered.

Input is one JSON object on stdin, followed by EOF:

```json
{"api_version":1,"action":"analyze","input":{"path":"/absolute/recording.cs8","format":"cs8","sample_rate":8000000,"center_hz":433920000}}
```

The file path is canonicalized. Formats are `cs8`, `cu8`, or `wav`; a decoder should reject unsupported formats. Input files remain on disk rather than being copied into the JSON payload. Treat external input as untrusted data.

Emit one JSON object on stdout and exit successfully. Reserve stderr for diagnostics (the host suppresses it). Recommended result fields are `api_version`, `decoder` or `identifier`, `events`, `confidence`, `sources`, and `notes`. Results are stored as JSON in SQLite's `findings` table. There is no enforced universal packet schema yet.

The host validates JSON, caps stdout at 1 MiB and enforces a process deadline. Addons run as the invoking user and inherit the environment: install only trusted code. The timeout is not a security sandbox or a resource isolation boundary. Descendant processes are not comprehensively sandboxed. Network access and external binaries are the module author's responsibility.

Examples in this repository:

- `ook-pulses`: standard-library Python envelope extractor, capped to one million IQ samples.
- `rtl433`: converts signed IQ to unsigned IQ if needed, then invokes `rtl_433` in file mode; 32 million sample cap and 45-second subprocess deadline.
- `band-context`: offline frequency candidate lookup with an EFIS source link. Does not assert a decoded protocol.

Future decoder/analyzer types can use this same subprocess contract without linking against a Rust ABI. For a new kind, extend discovery and the manifest validation in `src/addons.rs`.
