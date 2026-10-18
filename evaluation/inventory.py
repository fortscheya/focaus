"""Read-only asset fingerprints and bounded media metadata; never decodes or uploads."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path


def inspect(path):
    digest = hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda: source.read(8 * 1024 * 1024), b''):
            digest.update(block)
    result = subprocess.run([
        'ffprobe', '-v', 'error', '-show_entries',
        'format=duration,size,format_name:stream=index,codec_name,codec_type,sample_rate,channels,channel_layout,time_base,start_pts,start_time,duration_ts,duration',
        '-of', 'json', str(path)], capture_output=True, text=True, check=False)
    return {
        'asset_id': 'source_' + digest.hexdigest()[:16],
        'sha256': digest.hexdigest(), 'byte_length': path.stat().st_size,
        'metadata': json.loads(result.stdout) if result.returncode == 0 else None,
        'metadata_status': 'read' if result.returncode == 0 else 'unavailable',
        'original_preserved': True, 'managed_copy': None,
        'authorization': 'local read-only inspection; no cloud upload',
        'decode_status': 'not_tested', 'audition_status': 'not_tested',
        'corrupt_intervals': 'unknown', 'selected_track': None,
        'lesson_alignment': 'unconfirmed', 'derivatives': [],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('sources', nargs='+', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    records = [inspect(path) for path in args.sources]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps({'schema_version': 1, 'assets': records}, indent=2) + '\n')
    print(f'Inventoried {len(records)} original assets; no decode, selection, or upload performed.')


if __name__ == '__main__':
    main()
