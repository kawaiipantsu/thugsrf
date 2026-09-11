#!/usr/bin/env python3
"""Build a Debian package using dpkg-deb; no root, vendored SDK or cargo plugin needed."""
import argparse
import pathlib
import shutil
import subprocess
import tempfile
p = argparse.ArgumentParser()
p.add_argument('--version', required=True)
p.add_argument('--arch', required=True)
a = p.parse_args()
root = pathlib.Path(__file__).resolve().parent.parent
out = root / 'dist'
out.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix='thugsrf-package-') as temp:
    stage = pathlib.Path(temp)
    def copy(src, dst):
        target = stage / dst
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / src, target)
    copy('target/release/thugsrf', 'usr/bin/thugsrf')
    copy('docs/thugsrf.1', 'usr/share/man/man1/thugsrf.1')
    copy('README.md', 'usr/share/doc/thugsrf/README.md')
    copy('LICENSE', 'usr/share/doc/thugsrf/copyright')
    shutil.copytree(root / 'docs', stage / 'usr/share/doc/thugsrf/docs')
    shutil.copytree(root / 'assets', stage / 'usr/share/doc/thugsrf/assets')
    copy('assets/logo.txt', 'usr/share/thugsrf/logo.txt')
    shutil.copytree(root / 'addons', stage / 'usr/share/thugsrf/addons')
    (stage / 'DEBIAN').mkdir()
    # Let Debian derive the actual libc/libgcc requirements from this build.
    with tempfile.TemporaryDirectory(prefix='thugsrf-shlibs-') as check:
        check = pathlib.Path(check)
        (check / 'debian').mkdir()
        (check / 'debian/control').write_text('Source: thugsrf\nSection: science\nPriority: optional\nMaintainer: Kawaiipantsu <kawaiipantsu@users.noreply.github.com>\n\nPackage: thugsrf\nArchitecture: any\nDescription: radio workbench\n')
        result = subprocess.run(['dpkg-shlibdeps', '-O', '-e' + str(root / 'target/release/thugsrf')], cwd=check, text=True, capture_output=True, check=True)
        dependencies = result.stdout.strip().removeprefix('shlibs:Depends=')

    size = sum(f.stat().st_size for f in stage.rglob('*') if f.is_file()) // 1024
    (stage / 'DEBIAN/control').write_text(f'''Package: thugsrf
Version: {a.version}
Section: science
Priority: optional
Architecture: {a.arch}
Maintainer: Kawaiipantsu (THUGS(red)) <kawaiipantsu@users.noreply.github.com>
Homepage: https://thugs.red
Depends: {dependencies}, ca-certificates, hackrf, alsa-utils
Recommends: rtl-sdr, rtl-433, python3
Installed-Size: {size}
Description: THUGS(red) RF radio signal intelligence workbench
 Native Rust CLI and responsive TUI for HackRF and RTL-SDR reception,
 spectrum and waterfall analysis, modular decoders, IQ capture and replay,
 audio processing, SQLite investigations and optional AI analysis.
''')
    subprocess.run(['dpkg-deb', '--root-owner-group', '--build', str(stage), str(out / f'thugsrf_{a.version}_{a.arch}.deb')], check=True)
