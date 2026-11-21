#!/usr/bin/env python3
"""Validate native blend probe artifacts and summarize independent geometry/color evidence."""
import collections
import json
from pathlib import Path
import sys
from PIL import Image


def validate(directories):
    """Check every raw/PNG contract without converting close pixels into accepted parity."""
    summaries = []
    canonical = None
    for directory in directories:
        root = Path(directory)
        result = json.loads((root / 'results.json').read_text())
        assert len(result['geometry']) == 80 and len(result['colors']) in (6, 10)
        cpu_files = {p.name: p.read_bytes() for p in root.glob('*.cpu.rgba')}
        svg_files = {p.name: p.read_bytes() for p in root.glob('*.svg')}
        if canonical is not None:
            assert canonical == (cpu_files, svg_files), 'CPU/canonical inputs differ by route'
        canonical = cpu_files, svg_files
        by_mode = collections.defaultdict(list)
        colors = []
        for row in result['geometry'] + result['colors']:
            name = row['name']
            native = (root / (name + '.gpu-native.rgba')).read_bytes()
            straight = (root / (name + '.gpu-straight-carrier.rgba')).read_bytes()
            encoded = (root / (name + '.gpu-encoded.rgba')).read_bytes()
            assert len(native) == len(straight) == len(encoded) == 64 * 64 * 4
            assert native[3::4] == straight[3::4] == encoded[3::4]
            if result['route'].startswith('compute'):
                assert native == straight, 'compute native output is already straight'
            assert Image.open(root / (name + '.gpu.png')).convert('RGBA').tobytes() == encoded
            if 'submission' in row:
                by_mode[row['submission'] + '/' + row['mode']].append(row)
            else:
                cpu = cpu_files['color-' + row['model'] + '.cpu.rgba']
                visible = [(a, b) for a, b in zip(_pixels(cpu), _pixels(encoded))
                           if a[3] > 0 or b[3] > 0]
                colors.append(dict(name=name,
                                   max_visible_rgb_difference=max(abs(a[i] - b[i])
                                                                  for a, b in visible for i in range(3)),
                                   max_alpha_difference=row['max_alpha_byte_difference'],
                                   cpu_interior=row['cpu_pixel16_16'],
                                   gpu_interior=row['gpu_pixel16_16']))
        summaries.append(dict(route=result['route'], checked_artifacts=len(result['geometry']) + len(result['colors']),
                              geometry_modes={key: dict(controls=len(rows),
                                                        all_byte_equal=all(r['byte_equal'] for r in rows),
                                                        all_alpha_equal=all(r['different_alpha_pixels'] == 0
                                                                            for r in rows))
                                              for key, rows in by_mode.items()},
                              color_diagnostics=colors))
    return summaries


def _pixels(data):
    """Yield complete native-size RGBA pixels for independent visible-color comparison."""
    return (data[i:i + 4] for i in range(0, len(data), 4))


if __name__ == '__main__':
    print(json.dumps(validate(sys.argv[1:]), indent=2))
