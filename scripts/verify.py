"""Run release checks and smoke-test the offline archive/installer; records only executed checks."""
from pathlib import Path
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    checks = []
    logs = []

    def run(command, cwd=ROOT):
        result = subprocess.run(command, cwd=cwd, text=True, encoding='utf-8', errors='replace', capture_output=True)
        checks.append({'command': command, 'exit_code': result.returncode})
        logs.append(' '.join(command) + '\n' + result.stdout + result.stderr)
        print('PASS' if result.returncode == 0 else 'FAIL', ' '.join(command), flush=True)
        if result.returncode:
            print(result.stdout, result.stderr)
            raise RuntimeError('verification failed')
        return result.stdout

    result = {'platform': platform.platform(), 'checks': checks}
    report = ROOT / 'docs/development/delivery-test-results.json'
    try:
        result['rustc'] = run(['rustc', '--version']).strip()
        run(['cargo', 'fmt', '--check'])
        run(['cargo', 'clippy', '--locked', '--all-targets', '--', '-D', 'warnings'])
        tests = run(['cargo', 'test', '--locked'])
        result['tests_passed'] = sum(map(int, re.findall(r'test result: ok\. (\d+) passed', tests)))
        result['tests_failed'] = 0
        run(['cargo', 'build', '--release', '--locked'])
        package = json.loads(run([sys.executable, 'scripts/package.py', '--skip-build']))
        archive = Path(package['archive'])
        assert hashlib.sha256(archive.read_bytes()).hexdigest() == package['sha256']
        with tempfile.TemporaryDirectory(prefix='RepoRinse smoke ñ ') as directory:
            folder = Path(directory)
            if archive.suffix == '.zip':
                with zipfile.ZipFile(archive) as bundle:
                    assert all(not Path(n).is_absolute() and '..' not in Path(n).parts for n in bundle.namelist())
                    bundle.extractall(folder)
            else:
                with tarfile.open(archive) as bundle:
                    assert all(not Path(n).is_absolute() and '..' not in Path(n).parts for n in bundle.getnames())
                    bundle.extractall(folder, filter='data')
            extracted = next(folder.iterdir())
            exe = 'reporinse.exe' if os.name == 'nt' else 'reporinse'
            run([str(extracted / exe), '--version'], folder)
            destination = folder / 'installed bin'
            if os.name == 'nt':
                run([shutil.which('pwsh') or 'powershell.exe', '-NoProfile', '-NonInteractive', '-File', str(extracted / 'installers/install.ps1'), '-Binary', str(extracted / exe), '-Destination', str(destination)], folder)
            else:
                run(['sh', str(extracted / 'installers/install.sh'), str(extracted / exe), str(destination)], folder)
            run([str(destination / exe), '--version'], folder)
            assert hashlib.sha256((destination / exe).read_bytes()).digest() == hashlib.sha256((extracted / exe).read_bytes()).digest()
        result['archive'] = archive.name
        result['sha256'] = package['sha256']
        result['release_smoke'] = 'passed'
    finally:
        # Shareable report: remove machine-specific path prefixes from command records and logs.
        text = json.dumps(result, indent=2, ensure_ascii=False)
        log = '\n\n'.join(logs)
        for prefix in [str(ROOT), str(Path.home()), sys.executable]:
            text = text.replace(json.dumps(prefix)[1:-1], '<local>')
            log = log.replace(prefix, '<local>')
        report.parent.mkdir(parents=True, exist_ok=True)
        report.write_text(text + '\n', encoding='utf-8')
        (report.parent / 'delivery-tests.log').write_text(log, encoding='utf-8')


if __name__ == '__main__':
    main()
