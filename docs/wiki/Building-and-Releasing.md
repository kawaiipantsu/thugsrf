# Building, testing and documentation

The source repository carries the application, addons, build scripts, Debian packaging and wiki sources.

| Target | Purpose |
|---|---|
| make build | Optimized native binary with locked dependencies |
| make debug | Debug binary |
| make check | Formatting, strict Clippy and Rust tests |
| make test | Full CLI, terminal, receiver, protocol, reference and audio regression suite |
| make deb | Native Debian package under dist/ |
| make install | Install with PREFIX/DESTDIR overrides |
| make addons | Copy bundled addons without overwriting user files |
| make protocol-deps | Optional protocol backends |

The tuning regression uses a real pseudo-terminal and a fake HackRF driver, checking frequency arguments and exclusive access. Synthetic RX/TX tests do not radiate RF. Hardware validation and protocol-specific limits are documented in [[Verification]].

## Update this wiki

Edit `docs/wiki/` and the topic documents under `docs/` in the main repository. Generate a wiki checkout:

```sh
git clone https://github.com/kawaiipantsu/thugsrf.wiki.git /tmp/thugsrf-wiki
make build
python3 scripts/build-wiki.py /tmp/thugsrf-wiki
cd /tmp/thugsrf-wiki
git diff --check
git add .
git commit -m 'Update user manual'
git push
```

Generation copies the actual terminal screenshots and cropped branding, maps topic links to wiki pages and obtains command help from the built binary. Review changes before publication. The generator does not push by itself.
