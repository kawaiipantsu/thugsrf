# SigID Wiki reference intelligence

```sh
thugsrf sigid sync
thugsrf sigid status
thugsrf sigid lookup --frequency 153000000 --bandwidth 9000 --modulation FSK --mode NFM
thugsrf addon enable sigid-id
thugsrf addon run sigid-id recording.cs8 --frequency 153000000 --sample-rate 8000000
thugsrf identify recording.cs8 --frequency 153000000 --sample-rate 8000000
```

These commands also work in the TUI command bar. `sync` is the only operation that contacts SigID Wiki. It uses the public Semantic MediaWiki API with pagination, request/deadline limits and a transactional database replacement. A failed/empty refresh leaves the previous catalog intact. Normal lookup and addon matching work offline.

The cache is `~/.local/share/thugsrf/references.sqlite3`, with separate `sigid` and `reference_meta` tables. `XDG_DATA_HOME` is respected. Records retain the signal name, original frequency/bandwidth values, modulation, receiver mode, location labels and individual source-page URL; the catalog records its retrieval time and source. On 2026-09-11, **586 usable signal entries** were imported on the development host. The downloaded database is user data and is not bundled into the Git repository or Debian package.

The identifier estimates a dominant spectral peak and approximate occupied bandwidth from up to one million IQ samples. Override with `--options '{"bandwidth_hz":9000,"modulation":"FSK","mode":"NFM"}'` when better measurements are available. The standalone lookup accepts known values without needing a recording. Rank scores combine frequency-envelope specificity, bandwidth similarity and optional modulation/mode label matches. They are ordering scores, not probabilities, decoded packets or proof of signal identity. Mixed signals, DC carriers and interference can mislead the bandwidth estimate.

Frequency endpoints in the source can summarize broad or non-contiguous uses. They are not national allocations or station ownership records. Values are not silently corrected: suspicious unit/range ratios receive quality flags and cannot create huge automatic frequency-envelope matches. Follow the individual source page to verify an interesting candidate. For example, frequency plus 9 kHz bandwidth, FSK and NFM ranks POCSAG much more usefully than frequency alone.

The integration imports **factual structured metadata only**. It does not copy article prose, images, audio samples or IQ archives. The site's MediaWiki `rightsinfo` response did not specify a reuse license when checked; source attribution is retained and media/content reuse should be checked on the original pages. No blanket open-content license is assumed.

Source: [SigID Wiki Database](https://www.sigidwiki.com/wiki/Database), through its public `api.php?action=ask` endpoint. The local test suite mocks pagination and failures, verifies atomic replacement and ranking, and checks ambiguous-unit handling without requiring network access.
