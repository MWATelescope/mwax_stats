#
# Quick and dirty python matplotlib code to read the fringes binary file produced by mwax_stats and plot a baseline
#
# Usage:
# 
# python plot.py FILENAME BASELINE_NUMBER
#
#

import sys
import struct
import os
import numpy as np
import matplotlib.pyplot as plt

full_filename = sys.argv[1]
filename = os.path.basename(full_filename)

if len(sys.argv) == 2:
    view_baseline = 0
else:
    view_baseline = int(sys.argv[2])
print(f"Viewing plot for baseline {view_baseline}")

# filename example is 1319371344_fringes_128chans_128T_ch169.dat
if filename[21].isdigit():
    fine_chans = filename[19:22]
else:
    if filename[20].isdigit():
        fine_chans = filename[19:21]
    else:
        fine_chans = filename[19:20]

fine_chans = int(fine_chans)
print(f"Fine chans = {fine_chans}")

if filename[30].isdigit():
    tiles = filename[28:31]
else:
    if filename[29].isdigit():
        tiles = filename[28:30]
    else:
        tiles = filename[28:29]

tiles = int(tiles)
print(f"Tiles = {tiles}")
baselines = int((tiles * (tiles + 1)) / 2)
print(f"Baselines = {baselines}")

np_data = np.zeros((baselines, fine_chans, 2), dtype=float)
freqs = np.zeros(fine_chans, dtype=float)

with open(full_filename, "rb") as file:
    bl = 0
    fc = 0 
    bytearray = file.read(12)

    while bytearray:        
        np_data[bl, fc, 0] = struct.unpack('fff', bytearray)[1]
        np_data[bl, fc, 1] = struct.unpack('fff', bytearray)[2]

        if bl == 0:
            freqs[fc] = struct.unpack('fff', bytearray)[0]
        
        # read next triplet
        bytearray = file.read(12)
        fc = fc + 1

        if fc == fine_chans:
            bl += 1
            fc = 0

fig, ax = plt.subplots()
ax.set_title(f"Phases X (blue), Y (orange) for baseline {view_baseline}")
ax.set_ylim(ymin=-180, ymax=180)
ax.set_yticks(np.arange(-180, 180+1, 30.0))
ax.scatter(freqs, np_data[view_baseline, 0:fine_chans, 0])
ax.scatter(freqs, np_data[view_baseline, 0:fine_chans, 1])
plt.show()
