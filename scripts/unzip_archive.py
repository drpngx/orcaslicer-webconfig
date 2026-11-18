"""Hermetic, Zip-Slip-hardened ZIP extractor for build-time Bazel genrules."""

import os
import stat
import sys
import zipfile


def safe_extract_all(zip_path: str, dest_dir: str) -> None:
  real_dest = os.path.realpath(dest_dir)
  os.makedirs(real_dest, exist_ok=True)
  with zipfile.ZipFile(zip_path, "r") as zf:
    for info in zf.infolist():
      raw_name = info.filename.replace("\\", "/")
      if os.path.isabs(raw_name) or ".." in raw_name.split("/"):
        raise ValueError(f"Unsafe archive entry path: {info.filename!r}")
      unix_mode = info.external_attr >> 16
      if stat.S_ISLNK(unix_mode):
        raise ValueError(f"Symlink entries are forbidden in archive: {info.filename!r}")
      target_path = os.path.realpath(os.path.join(real_dest, raw_name))
      if target_path != real_dest and not target_path.startswith(real_dest + os.sep):
        raise ValueError(f"Zip-Slip path traversal detected: {info.filename!r}")
      if info.is_dir() or raw_name.endswith("/"):
        os.makedirs(target_path, exist_ok=True)
      else:
        os.makedirs(os.path.dirname(target_path), exist_ok=True)
        with zf.open(info, "r") as src, open(target_path, "wb") as dst:
          dst.write(src.read())


def main() -> None:
  if len(sys.argv) != 3:
    sys.stderr.write("Usage: unzip_archive.py <input.zip> <output_dir>\n")
    sys.exit(1)
  safe_extract_all(sys.argv[1], sys.argv[2])


if __name__ == "__main__":
  main()
