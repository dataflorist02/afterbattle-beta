"""Checks the word list of the template summaries against SJP.PL (spec 30 section 7 point 4).

Every word of src/lib/recap/words.pl.txt must be an inflected form in SJP.PL "odmiany slow",
edition 2026-11-01 (sjp.pl/sl/odmiany, licence CC BY 4.0 for the list in the application). The
script never downloads anything: pass the extracted odm.txt, whose SHA-256 must match the
edition named in spec 28 section 6. CI does not run this script; the summary tests use the list
in the repository.

Usage: python scripts/check-recap-words.py <path to odm.txt> [path to words.pl.txt]
Exit code 0 when every word is in the dictionary, 1 when some are not, 2 on a wrong file.
"""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path

ODM_SHA256 = "8ba9ad794657b062841eaf384a423dd9f4ec83e6d59fce998cf80e1f271e08c1"
DEFAULT_WORDS = Path(__file__).resolve().parent.parent / "src" / "lib" / "recap" / "words.pl.txt"


def read_words(path: Path) -> set[str]:
    words = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        word = line.strip()
        if word and not word.startswith("#"):
            words.add(word.lower())
    return words


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def missing_words(words: set[str], odm: Path) -> set[str]:
    left = set(words)
    with open(odm, encoding="utf-8") as handle:
        for line in handle:
            for form in line.rstrip("\r\n").split(", "):
                left.discard(form.lower())
            if not left:
                break
    return left


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__)
        return 2
    odm = Path(argv[1])
    words_path = Path(argv[2]) if len(argv) > 2 else DEFAULT_WORDS
    actual = sha256(odm)
    if actual != ODM_SHA256:
        print(f"odm.txt SHA-256 {actual} is not the 2026-11-01 edition ({ODM_SHA256})")
        return 2
    words = read_words(words_path)
    missing = missing_words(words, odm)
    for word in sorted(missing):
        print(f"not in SJP.PL: {word}")
    print(f"{len(words) - len(missing)} of {len(words)} words found in SJP.PL")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
