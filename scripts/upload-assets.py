#!/usr/bin/env python3
"""Upload assets/original + manifest.json + full zip to Cloudflare R2 (S3 API).

Credentials come from the environment only (never commit them):
  R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY, R2_ENDPOINT, R2_BUCKET
e.g.  set -a; . ~/.config/lolping/r2.env; set +a; python3 scripts/upload-assets.py

Prebuilt set (e.g. v2 from scripts/build-v2-assets.sh, which writes manifest.json + zip):
  R2_PREFIX=v2 python3 scripts/upload-assets.py dist/assets-v2

Requires: pip install boto3
"""
import hashlib, io, json, os, sys, zipfile
from pathlib import Path

import boto3

ROOT = Path(__file__).resolve().parent.parent / "assets" / "original"
PREFIX = os.environ.get("R2_PREFIX", "v1")
ZIP = f"lolping-assets-{PREFIX}.zip"
TYPES = {".webp": "image/webp", ".png": "image/png", ".ogg": "audio/ogg",
         ".wav": "audio/wav", ".webm": "video/webm", ".json": "application/json", ".zip": "application/zip"}

def main():
    env = {k: os.environ.get(k) for k in ("R2_ACCESS_KEY_ID", "R2_SECRET_ACCESS_KEY", "R2_ENDPOINT", "R2_BUCKET")}
    if not all(env.values()):
        sys.exit("missing R2_* env vars")
    s3 = boto3.client("s3", endpoint_url=env["R2_ENDPOINT"], region_name="auto",
                      aws_access_key_id=env["R2_ACCESS_KEY_ID"],
                      aws_secret_access_key=env["R2_SECRET_ACCESS_KEY"])
    if len(sys.argv) > 1:
        return upload_prebuilt(s3, env["R2_BUCKET"], Path(sys.argv[1]))
    files, zbuf = [], io.BytesIO()
    with zipfile.ZipFile(zbuf, "w", zipfile.ZIP_DEFLATED) as z:
        for p in sorted(ROOT.rglob("*")):
            if not p.is_file() or p.name.startswith("."):
                continue
            rel = p.relative_to(ROOT).as_posix()
            data = p.read_bytes()
            files.append({"path": rel, "size": len(data), "sha256": hashlib.sha256(data).hexdigest()})
            z.writestr(rel, data)
            s3.put_object(Bucket=env["R2_BUCKET"], Key=f"{PREFIX}/{rel}", Body=data,
                          ContentType=TYPES.get(p.suffix, "application/octet-stream"),
                          CacheControl="public, max-age=31536000, immutable")
            print("up", rel)
    s3.put_object(Bucket=env["R2_BUCKET"], Key=f"{PREFIX}/{ZIP}", Body=zbuf.getvalue(),
                  ContentType=TYPES[".zip"], CacheControl="public, max-age=86400")
    manifest = json.dumps({"version": 1, "files": files}, indent=1).encode()
    s3.put_object(Bucket=env["R2_BUCKET"], Key=f"{PREFIX}/manifest.json", Body=manifest,
                  ContentType=TYPES[".json"], CacheControl="public, max-age=300")
    print(f"done: {len(files)} files + {ZIP} + manifest.json")

def upload_prebuilt(s3, bucket, root):
    m = json.loads((root / "manifest.json").read_text())
    for f in m["files"]:
        data = (root / f["path"]).read_bytes()
        assert len(data) == f["size"] and hashlib.sha256(data).hexdigest() == f["sha256"], f["path"]
        s3.put_object(Bucket=bucket, Key=f"{PREFIX}/{f['path']}", Body=data,
                      ContentType=TYPES.get(Path(f["path"]).suffix, "application/octet-stream"),
                      CacheControl="public, max-age=31536000, immutable")
        print("up", f["path"])
    s3.put_object(Bucket=bucket, Key=f"{PREFIX}/{ZIP}", Body=(root / ZIP).read_bytes(),
                  ContentType=TYPES[".zip"], CacheControl="public, max-age=31536000, immutable")
    s3.put_object(Bucket=bucket, Key=f"{PREFIX}/manifest.json", Body=(root / "manifest.json").read_bytes(),
                  ContentType=TYPES[".json"], CacheControl="public, max-age=300")
    print(f"done: {len(m['files'])} files + {ZIP} + manifest.json -> {PREFIX}/")


if __name__ == "__main__":
    main()
