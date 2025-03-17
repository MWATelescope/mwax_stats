# mwax_stats repo

This repo contains two programs:

* mwax_stats - used to summarise fringes and autocorrelations for plotting in M&C.
* mwax_packet_stats - used to summarise packet loss stats for each subfile for plotting in M&C.

Both `mwax_stats` and `mwax_packet_stats` utilises the `env_logger` and `log` crates, you can set different levels of logging output using the RUST_LOG variable:
```RUST_LOG=info|debug|trace mwax_stats...```. The `info` logging level is the least verbose, and `trace` is the most- it will output a line of debug for each unit of data written (you have been warned!).

## mwax_stats

The `mwax` correlator (via mwax_mover) exectutes mwax_stats for each visibility FITS file created that is marked as a calibrator. `mwax_stats` uses `mwalib` to read the visibilities then output various stats dumps which are used by the M&C system to provide near-realtime plots allowing a human to verify correlator output is ok.

### mwax_stats: Usage

```bash
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

### Auto-correlation output

`mwax_stats` will output power (in dB) for XX and YY vs frequency for all tiles for all provided coarse channels for the last timestep in the observation.

#### Auto-correlation: Filename

* filename = OOOOOOOOOO_autos_FFFFchans_NNNT_chRRR.dat e.g. `1317706936_autos_64chans_128T_ch123.dat` would describe obsid 1317706936, 64 fine channels per coarse and 128 tiles for receiver coarse channel 123.
  * Where:
    * OOOOOOOOOO = Obsid
    * FFFF = number of fine channels. Could be 1,2,3 or 4 digits depending on the correlator mode
    * NNN = number of tiles
    * RRR = receiver coarse channel number. Could be 1,2 or 3 digits

#### Auto-correlation: Output format

* for each tile:
  * for each fine channel:
    * 3 float32 values:
      * Frequency (MHz)
      * XX power (dB)
      * YY power (dB)
* Tiles are in "antenna" order

### Fringes output

`mwax_stats` will also output phase (XX and YY) vs frequency for all baselines for all provided coarse channels for the last timestep in the observation.

#### Fringes: Filename

* filename = OOOOOOOOOO_fringes_FFFFchans_NNNT_chRRR.dat e.g. `1317706936_fringes_64chans_128T_ch123.dat` would describe obsid 1317706936, 64 fine channels per coarse and 128 tiles for receiver coarse channel 123.
  * Where:
    * OOOOOOOOOO = Obsid
    * FFFF = number of fine channels. Could be 1,2,3 or 4 digits depending on the correlator mode
    * NNN = number of tiles
    * RRR = receiver coarse channel number. Could be 1,2 or 3 digits

#### Fringes: Output format

* for each baseline:
  * for each fine channel:
    * 3 float32 values:
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

## mwax_packet_stats

### mwax_packet_stats: Usage

`mwax_mover` will run mwax_packet_stats for each subfile that it gets from the `mwax_u2s` process. It will read the subfile, find the location of the packet stats information and then extract, summarise and output it to a file. `mwax_mover` will then, in a seperate thread lazily copy the stats file to it's final location on an NFS share hosted on vulcan. See: [this page](https://mwatelescope.atlassian.net/wiki/spaces/MP/pages/24970579/MWAX+PSRDADA+header) on the MWA wiki for more information about the packet stats region of block0 in the subfile.

```bash
USAGE:
    mwax_packet_stats -o <output-dir> -s <subfile_name>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -o <output-dir>          Specify the directory to write output files to.
    -s <subfile_name>        Sets the subfile name/path.
```

### mwax_packet_stats: Output format

* filename = packetstats_SSSSSSSS_NNNT_chCCC_MMM.dat e.g. `packetstats_1234567890_128T_ch123_mwax01.dat` would describe subobsid 1234567890, 128 tiles for receiver coarse channel 123 from mwax01.
  * Where:
    * SSSSSSSSSS = subobsid
    * NNNT = number of tiles- it is not zero padded. e.g. could be 1,2 or 3 digits and a "T"
    * CCC = receiver channel number- it is not zero padded. e.g. could be 1,2 or 3 digits
    * MMM = hostname
* data
  * Each data file contains a UINT16 `packets lost` count per rfinput (where the rfinputs are in the subfile order). 0 represents no packet loss over the full 8 seconds of that subobservation.
