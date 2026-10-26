"""Build a local release archive. Never publishes or accesses a hosting account."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    args = parser.parse_args()
    if not args.skip_build:
        subprocess.run(["cargo", "build", "--release", "--locked"], cwd=ROOT, check=True)
    exe = "reporinse.exe" if platform.system() == "Windows" else "reporinse"
    binary = ROOT / "target" / "release" / exe
    version = subprocess.check_output([str(binary), "--version"], text=True).strip().split()[-1]
    name = f"reporinse-{version}-{platform.system().lower()}-{platform.machine().lower()}"
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="reporinse-package-") as folder:
        bundle = Path(folder) / name
        bundle.mkdir()
        shutil.copy2(binary, bundle / exe)
        for filename in ["README.md", "README.es.md", "LICENSE", "CHANGELOG.md", "SECURITY.md"]:
            shutil.copy2(ROOT / filename, bundle / filename)
        for directory in ["docs", "examples", "installers"]:
            shutil.copytree(ROOT / directory, bundle / directory, ignore=shutil.ignore_patterns("delivery-test-results.json", "delivery-tests.log"))
        archive = Path(shutil.make_archive(str(output / name), "zip" if exe.endswith(".exe") else "gztar", folder, name))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    checksum = output / f"{archive.name}.sha256"
    checksum.write_text(f"{digest}  {archive.name}\n", encoding="ascii")
    print(json.dumps({"archive": str(archive), "sha256": digest, "checksum_file": str(checksum)}))


if __name__ == "__main__":
    main()
