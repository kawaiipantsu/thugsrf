# Frequency references and provenance

Press **l** in the TUI to look up the tuned frequency. The command bar supports all operations below. The default lookup region is Denmark (`DK`); choose `US` or `ALL` explicitly for other references.

```sh
thugsrf frequency sources
thugsrf frequency lookup --frequency 145600000 --tolerance 12500
thugsrf frequency lookup --frequency 100000000 --region US
thugsrf frequency import saved-dkscan-list.html --source http://www.dkscan.dk/frekvens.htm --region DK
thugsrf frequency import my-list.csv --source https://example.org/my-reference --region US
thugsrf addon enable danish-frequency
thugsrf addon run danish-frequency signal.cs8 --options '{"region":"DK","tolerance_hz":12500}'
```

Imports are merged and deduplicated into `~/.local/share/thugsrf/frequencies.json`, an editable file. Each entry retains source, region, import time and evidence label. Lookup returns distance from the listed channel/range and up to 100 nearest matches within tolerance. This is a service/user **hint**, never proof of transmitter ownership. Lists can be outdated and frequencies shared. The import date is not the source's publication date.

CSV headers may be `frequency_mhz,label,source,region` for channels or `low_hz,high_hz,label,source,region` for ranges. Source and region columns are optional when supplied on the command line. Example:

```csv
frequency_mhz,label,source,region
145.600,Example only - replace with verified entry,https://example.org/reference,DK
```

HTML import reads table rows containing a complete decimal-frequency cell, with optional MHz/kHz units or a range. Unitless decimal cells are treated as MHz. Review the resulting JSON against the source; arbitrary web layouts are not reliably machine-readable. Import the individual list pages, not just a site's menu. No network request occurs during import or lookup.

DKScan's HTTP endpoint blocked this development environment; its HTTPS hostname also failed certificate validation. No DKScan entries have been fetched, invented, or bundled. Save the relevant page in your browser and import it, or use a reviewed CSV export. The tool does not bypass the site's restrictions.

Sources exposed by `frequency sources`:

- [DKScan](http://www.dkscan.dk/frekvens.htm): Danish community frequency lists.
- [CEPT EFIS](https://efis.cept.org/): national allocations and applications, including Denmark.
- [NTIA allocation chart](https://www.ntia.gov/page/united-states-frequency-allocation-chart): US chart, September 2025 edition based on March 2025 data.
- [FCC §2.106](https://www.ecfr.gov/current/title-47/chapter-I/subchapter-A/part-2/subpart-B/section-2.106): current US allocation table.
- [FCC public files](https://publicfiles.fcc.gov/): broadcast station references.

A small built-in US starter set covers AM/FM broadcasting, VHF aviation and three amateur bands, with individual FCC source links. It is explicitly incomplete. European protocol-family hints are also provided by the separate identifier addons. Neither set is a complete national allocation database. Google can help locate references; search snippets are not imported as authoritative allocation data. There is no automatic web search or periodic refresh.
