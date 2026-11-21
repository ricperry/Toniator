#!/usr/bin/env python3
"""Generate Toniator's project-owned periodic void-and-cluster rank permutation.

Run with Python 3 and NumPy. No upstream code or texture is copied. Fixed xorshift
initial occupancy, Gaussian toroidal energy, deterministic first-index ties, and
cluster removal / void insertion produce all 4096 unique progressive ranks.
The checked-in table is runtime authority; regeneration requires reviewing its
hash and sparse-tone spatial evidence (NumPy/libm roundoff may affect ties).
"""
from pathlib import Path
import hashlib
import numpy as np

N = 64
sigma = 1.5
axis = np.minimum(np.arange(N), N - np.arange(N))
kernel = np.exp(-(axis[:, None] ** 2 + axis[None, :] ** 2) / (2 * sigma ** 2))
state = 0x4F4E4931
keys = []
for i in range(N*N):
    state ^= (state << 13) & 0xffffffff
    state ^= state >> 17
    state ^= (state << 5) & 0xffffffff
    state &= 0xffffffff
    keys.append((state, i))
occupied = np.zeros((N, N), dtype=bool)
for _, i in sorted(keys)[:N*N//10]:
    occupied.flat[i] = True

def influence(i):
    return np.roll(kernel, (i // N, i % N), axis=(0, 1))

def energy(mask):
    return sum((influence(i) for i in np.flatnonzero(mask)), np.zeros((N, N)))

def cluster(mask, field):
    return int(np.argmax(np.where(mask, field, -np.inf)))

def void(mask, field):
    return int(np.argmin(np.where(mask, np.inf, field)))

field = energy(occupied)
for iteration in range(100000):
    i = cluster(occupied, field)
    occupied.flat[i] = False
    field -= influence(i)
    j = void(occupied, field)
    occupied.flat[j] = True
    field += influence(j)
    if i == j:
        break
else:
    raise RuntimeError('initial cluster relaxation did not converge')
initial = occupied.copy()
ranks = np.zeros((N, N), dtype=np.uint16)
for rank in range(int(occupied.sum())-1, -1, -1):
    i = cluster(occupied, field)
    ranks.flat[i] = rank
    occupied.flat[i] = False
    field -= influence(i)
occupied = initial.copy()
field = energy(occupied)
for rank in range(int(occupied.sum()), N*N):
    i = void(occupied, field)
    ranks.flat[i] = rank
    occupied.flat[i] = True
    field += influence(i)
assert sorted(ranks.flat) == list(range(N*N))
raw = ranks.astype('<u2').tobytes()
output = Path(__file__).resolve().parents[1] / 'crates/toniator-patterns/src/blue_noise_64.bin'
output.write_bytes(raw)
print(f'{output}: sha256={hashlib.sha256(raw).hexdigest()} relaxation_iterations={iteration+1}')
