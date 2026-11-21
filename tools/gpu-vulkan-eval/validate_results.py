#!/usr/bin/env python3
"""Validate actual native readback conventions and canonical cross-route controls."""
import json
from pathlib import Path
import sys
from PIL import Image


def validate(directories):
    """Reject output-format/harness mistakes without treating pixel equality as certification."""
    identities = None
    svg_bytes = None
    reports = []
    for directory in directories:
        root = Path(directory)
        result = json.loads((root / 'results.json').read_text())
        witnesses = {item['name']: item for item in result['witnesses']}
        assert len(witnesses) == 12, 'complete bounded matrix required'
        actual = {name: item['canonical_scene_identity'] for name, item in witnesses.items()}
        svgs = {name: (root / (name + '.svg')).read_bytes() for name in witnesses}
        if identities is not None:
            assert identities == actual, 'canonical scene identities differ between routes'
            assert svg_bytes == svgs, 'canonical SVG outputs differ between routes'
        identities, svg_bytes = actual, svgs
        for name, item in witnesses.items():
            straight = (root / (name + '.gpu-straight.rgba')).read_bytes()
            native = (root / (name + '.gpu-native.rgba')).read_bytes()
            assert len(native) == len(straight) == 64 * 64 * 4
            assert Image.open(root / (name + '.gpu.png')).convert('RGBA').tobytes() == straight
            if result['route'].startswith('compute'):
                assert native == straight, 'compute shader already writes straight alpha'
            assert native[3::4] == straight[3::4], 'unassociation must preserve alpha'
        clip = (root / 'fractional_clip.gpu-native.rgba').read_bytes()
        png = (root / 'fractional_clip.gpu-straight.rgba').read_bytes()
        # This exact white quarter-pixel witness verifies alpha/color conventions,
        # independent of the complicated union/curve behavior under evaluation.
        assert list(png[:4]) == [255, 255, 255, 64]
        assert list(clip[:4]) == ([64, 64, 64, 64] if result['route'] == 'hybrid'
                                 else [255, 255, 255, 64])
        assert witnesses['touching_halves']['byte_equal'], 'full-coverage control failed'
        assert witnesses['overlap_union']['cpu_first_pixel'] == [255, 255, 255, 230]
        if result['route'] == 'hybrid':
            assert witnesses['preunion_control']['byte_equal'], 'hybrid preunion control failed'
        reports.append(dict(route=result['route'], witnesses=len(witnesses),
                            native_format_contract='pass', canonical_identity_contract='pass',
                            candidate_parity='not granted by this validator'))
    return reports


if __name__ == '__main__':
    print(json.dumps(validate(sys.argv[1:]), indent=2))
