# mwax_stats
The `mwax` correlator exectutes mwax_stats for each visibility FITS file created. `mwax_stats` uses `mwalib` to read the visibilities then output various stats dumps which are used by the M&C system to provide near-realtime plots allowing a human to verify correlator output is ok. 

## Usage
```
USAGE:
    mwax_stats <fits-files>... -m <metafits> -o <output-dir>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -m <metafits>          Sets the metafits file.
    -o <output-dir>        Specify the directory to write output files to.

ARGS:
    <fits-files>...
```
Since `mwax_stats` utilises the `env_logger` and `log` crates, you can set different levels of logging output using the RUST_LOG variable:
```RUST_LOG=info|debug|trace mwax_stats...```. The `info` logging level is the least verbose, and `trace` is the most- it will output a line of debug for each unit of data written (you have been warned!).

## Auto-correlation output
`mwax_stats` will output power (in dB) for XX and YY vs frequency for all tiles for all provided coarse channels for the last timestep in the observation.

### Filename
* filename = OOOOOOOOOO_autos_FFFFchans_NNNT.dat e.g. `1317706936_autos_64chans_128T.dat` would describe obsid 1317706936, 64 fine channels per coarse and 128 tiles.
    * Where:
        * OOOOOOOOOO = Obsid
        * FFFF = number of fine channels. Could be 1,2,3 or 4 digits depending on the correlator mode
        * NNN = number of tiles

### Output format
* 3 float32s per tile: 
    * Frequency (MHz)
    * XX power (dB)
    * YY power (dB)
* Tiles are in "antenna" order

## Fringes output
`mwax_stats` will also output phase (XX and YY) vs frequency for all baselines for all provided coarse channels for the last timestep in the observation.

### Filename
* filename = OOOOOOOOOO_fringes_FFFFchans_NNNT.dat e.g. `1317706936_fringes_64chans_128T.dat` would describe obsid 1317706936, 64 fine channels per coarse and 128 tiles.
    * Where:
        * OOOOOOOOOO = Obsid
        * FFFF = number of fine channels. Could be 1,2,3 or 4 digits depending on the correlator mode
        * NNN = number of tiles

### Output format
* 3 float32 per baseline:
    * Frequency (MHz)
    * XX phase (degrees)
    * YY phase (degrees)
* Baselines are in lower right triangular order with tile1 vs tile2. Example below for 128 tiles: 
    * 0 v 0 
    * 0 v 1 
    * ...
    * 0 v 127
    * 1 v 1
    * 1 v 2
    * ...
    * 1 v 127
    * ...
    * 127 v 127
* Tiles are in "antenna" order