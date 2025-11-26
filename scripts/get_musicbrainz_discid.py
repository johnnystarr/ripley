#!/usr/bin/env python3
"""
Simple wrapper to calculate MusicBrainz disc ID from a CD device
Uses python-discid library which wraps libdiscid
"""
import sys

try:
    import discid
except ImportError:
    print("ERROR: python-discid not installed", file=sys.stderr)
    print("Install with: pip3 install --break-system-packages discid", file=sys.stderr)
    sys.exit(1)

if len(sys.argv) < 2:
    print(f"Usage: {sys.argv[0]} <device>", file=sys.stderr)
    sys.exit(1)

device = sys.argv[1]

try:
    disc = discid.read(device)
    print(disc.id)
except discid.DiscError as e:
    print(f"ERROR: {e}", file=sys.stderr)
    sys.exit(1)
