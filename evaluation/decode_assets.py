"""Decode each audio track separately to null; never combine, save audio, or upload."""
import argparse
import json
import subprocess
import time
from pathlib import Path
from inventory import inspect

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('sources', nargs='+', type=Path)
p.add_argument('--output', required=True, type=Path)
a = p.parse_args()
records = []
for source in a.sources:
    asset = inspect(source)
    results = []
    for stream in (asset.get('metadata') or {}).get('streams', []):
        if stream['codec_type'] != 'audio':
            continue
        started = time.monotonic()
        r = subprocess.run(['ffmpeg', '-nostdin', '-hide_banner', '-v', 'error', '-xerror', '-threads', '1', '-i', str(source), '-map', f"0:{stream['index']}", '-progress', 'pipe:1', '-f', 'null', '-'], capture_output=True, text=True)
        progress = dict(line.split('=', 1) for line in r.stdout.splitlines() if '=' in line)
        # Do not persist raw stderr: it can include local paths and media tags.
        results.append({'stream_index': stream['index'], 'codec': stream.get('codec_name'), 'status': 'decoded_to_null' if r.returncode == 0 else 'decode_failed', 'exit_code': r.returncode, 'decoded_out_time_us': progress.get('out_time_us'), 'wall_seconds': round(time.monotonic()-started,3), 'error_class': None if r.returncode == 0 else ('decoder_unavailable' if 'not found' in r.stderr.lower() or 'no decoder found' in r.stderr.lower() or 'unknown decoder' in r.stderr.lower() else 'decode_or_environment_error'), 'audio_saved': False})
    records.append({'asset_id': asset['asset_id'], 'source_sha256': asset['sha256'], 'tracks':results, 'track_selected':None, 'audition':'not_tested', 'lesson_alignment':'unconfirmed'})
a.output.parent.mkdir(parents=True, exist_ok=True)
a.output.write_text(json.dumps({'schema_version':1,'mode':'local_each_track_to_null','assets':records},indent=2)+'\n')
print(json.dumps(records,indent=2))
