#!/usr/bin/env python3
"""Compatibility entry point; maintained code lives in tailscale-gateway."""
import os
from pathlib import Path
import runpy
here = Path(__file__).resolve().parent
os.environ.setdefault("U60_GATEWAY_ARTIFACT_DIR", str(here))
runpy.run_path(str(here.parent / "tailscale-gateway/deploy.py"), run_name="__main__")
