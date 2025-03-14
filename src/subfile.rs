// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::path::Path;
use std::fs::File;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::str;
use anyhow::{anyhow, Result};
use ndarray::Array;
use log::{debug,info};
use rayon::prelude::*;

const PSRDADA_HEADER_LEN:usize = 4096;
const KEY_SUBOBS_ID: &str = "SUBOBS_ID";
const KEY_IDX_PACKET_MAP: &str = "IDX_PACKET_MAP";
const KEY_NINPUTS: &str = "NINPUTS";
const KEY_COARSE_CHANNEL: &str = "COARSE_CHANNEL";

struct PsrdadaHeader {
    map_start_index: u64,
    map_length: usize,
    ninputs: usize,
    subobs_id: String,
    chan: String
}

/// Reads the packet stats from a subfile and writes a count of lost packets per input (1 tile=2 inputs)
///
/// # Arguments
///
/// * `subfile_name` - Reference to path to the subfile to read
/// 
/// * `output_dir` - Reference to the path to the output dir to write the file (the filename is generated)
/// 
/// * `hostname` - Reference to a string containing the hostname (used when generating the output filename)
/// 
///
/// # Returns
///
/// * Result - Ok on success (and file written), or an error on failure
/// 
pub(crate) fn process_subfile_packet_map_data(subfile_name: &Path, output_dir: &Path, hostname: &str) -> Result<(), anyhow::Error> {    
    // Open the subfile    
    let mut file = File::open(subfile_name)?;

    // Process the header to get the info we want
    let info: PsrdadaHeader = read_psrdada_header(&mut file)?;
    
    // Create a buffer for the counts
    let mut packets_lost: Vec<u16> = vec!(0; info.ninputs);

    // Read packet map from file and populate the packet map array
    read_packet_map(&mut file, info.ninputs, info.map_start_index, info.map_length, &mut packets_lost)?;

    // Determine output filename
    let output_filename = output_dir.join(format!("packetstats_{}_{}T_ch{}_{}.dat", info.subobs_id, info.ninputs/2, info.chan, hostname));

    // Write file
    write_packet_stats(&packets_lost, &output_filename)?;

    info!("Successfully wrote packet stats to: {}", output_filename.display());

    Ok(())
}

fn read_packet_map(file: &mut File, ninputs: usize, map_start_index: u64, map_length: usize, packets_lost: &mut [u16]) -> Result<(),anyhow::Error> {
    // Allocate a buffer
    let mut buf = vec![0_u8; map_length];
    
    // Read the data from Block0 of the file
    // We should already be at the start of Block0 from reading the header, but seek to start of packet map
    file.seek(SeekFrom::Start(PSRDADA_HEADER_LEN as u64 + map_start_index))?;
    file.read_exact(&mut buf)?;
        
    // Determine max number of packets
    let num_bytes_per_input = buf.len() / ninputs;
        
    // Change packet_map into a 2d array by rf_input
    let packet_map = Array::from_shape_vec((ninputs, num_bytes_per_input), buf)?;    
        
    // Loop through each input in parallel
    packets_lost.par_iter_mut().enumerate().for_each(| (input, v)| {                
        // For each input we have a whole bunch of byte sized bitmaps e.g. 0001000,11110111,etc
        // e.g. a value of 00000000 has 8 lost packets. A value of 00000001 has 7 lost packets bit, a value of 01010101 has 4 lost packets.        
        for b in 0..num_bytes_per_input {
            // Accumulate the count of bits.
            *v += packet_map[[input, b]].count_zeros() as u16;
        }                
    });

    Ok(())
}

/// Write packet stats to disk
///
/// # Arguments
///
/// * `packets_lost` - Reference to array or slice of u16's representing packets lost counts (1 element per input)
/// 
/// * `output_filename`- filename to write to as a `Path` reference
/// 
///
/// # Returns
///
/// * Result - Ok on success, or an error on failure
///
fn write_packet_stats(packets_lost: &[u16], output_filename: &Path) -> Result<(),anyhow::Error>{
    // Now write the data file    
    let mut out_file:  File = File::create(output_filename)?;
    
    // Write (after converting the uint16's to 2 bytes (as little endian))
    let _ = out_file.write_all(&packets_lost.iter().flat_map(|int| int.to_le_bytes()).collect::<Vec<u8>>());
    out_file.flush()?;
    
    Ok(())
}

fn read_psrdada_header(file: &mut File)-> Result<PsrdadaHeader,anyhow::Error> {
    // Read header into local buffer    
    let mut header_buf = [0_u8; PSRDADA_HEADER_LEN];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut header_buf)?;
    
    // Convert the bytes into a UTF-8 string
    let contents: Vec<&str> = str::from_utf8(&header_buf)?.split("\n").collect();
    
    // Read header and get the packet stats indices    
    let packet_stats_idx = read_subfile_header_key(&contents, KEY_IDX_PACKET_MAP)?;
    let ninputs: usize = read_subfile_header_key(&contents, KEY_NINPUTS)?.parse()?;
    let subobs_id = read_subfile_header_key(&contents, KEY_SUBOBS_ID)?;
    let chan = read_subfile_header_key(&contents, KEY_COARSE_CHANNEL)?;
    
    // IDX_PACKET_MAP contains X+Y where X is the start byte of Block0 and Y is the length
    let (start, length) = packet_stats_idx.split_once("+").unwrap_or(("",""));

    // Parse the start and length to usize
    let map_start_index:u64 = start.parse()?;
    let map_length:usize = length.parse()?;

    Ok( PsrdadaHeader { map_start_index, map_length, ninputs, subobs_id, chan } )
}

/// Given the contents of the PSRDADA header, return the value of a given key.
///
/// # Arguments
///
/// * `header` - An array of lines of text which is the PSRDADA header
/// 
/// * `key`- String value of the key to look for
///
///
/// # Returns
///
/// * Result - containing the string value on success, or an error on failure (or key not found)
///
fn read_subfile_header_key(header: &Vec<&str>, key: &str) -> Result<String, anyhow::Error> {                
    // Split line into key<space>value
    // If key matches, return it and the value
    // Otherwise keep looking
    for line in header {        
        let (found_key, value) = line.split_once(" ").unwrap_or(("",""));

        if found_key == key {
            debug!("Read {}={}", key, value);
            return Ok(value.to_string());
        }
    }

    // If we get here we did not find the key
    Err(anyhow!("failed to find key {} in subfile", key))
}

#[cfg(test)]
mod tests {
    use crate::subfile::*;

    #[test]
    fn test_read_subfile_header_key_ok1() {    
        let test_header = ["ABC 123","DEF test","TEST3",""].to_vec();

        assert_eq!(read_subfile_header_key(&test_header, "ABC").expect("error"), "123");
    }

    #[test]
    fn test_read_subfile_header_key_ok2() {    
        let test_header = ["ABC 123","DEF test","TEST3",""].to_vec();

        assert_eq!(read_subfile_header_key(&test_header, "DEF").expect("error"), "test");
    }

    #[test]
    fn test_read_subfile_header_key_error_missing_value() {    
        let test_header = ["ABC 123","DEF test","TEST3",""].to_vec();

        assert!(read_subfile_header_key(&test_header, "TEST3").is_err());
    }

    #[test]
    fn test_read_subfile_header_error_key_not_found() {    
        let test_header = ["ABC 123","DEF test","TEST3",""].to_vec();

        assert!(read_subfile_header_key(&test_header, "unknown_key").is_err());
    }

    #[test]
    fn test_write_packet_stats() {    
        // Setup
        let filename = "/tmp/tmp_packet_stats.dat";
        
        // first u16= {8, 0} == 8
        // second u16={50,8} == 2098
        let packets_lost: Vec<u16> = [8, 2098].to_vec();
        
        // Do the write
        write_packet_stats(&packets_lost, Path::new(filename)).unwrap();

        // Reread file and check
        let mut buf = vec![0_u8; 4];
        let mut f = File::open(filename).unwrap();
        f.read_exact(&mut buf).unwrap();

        // Compare bytes
        assert_eq!(buf[0], 8);
        assert_eq!(buf[1], 0);
        assert_eq!(buf[2], 50);
        assert_eq!(buf[3], 8);
    }

    #[test]
    fn test_read_psrdada_header() {
        let filename = "test_files/1419789248_1419789248_91_small.sub";

        // Open file
        let mut file = File::open(filename).unwrap();

        let p = read_psrdada_header(&mut file).unwrap();

        assert_eq!(p.chan, "91");
        assert_eq!(p.map_start_index, 6351360);
        assert_eq!(p.map_length, 150000);
        assert_eq!(p.ninputs, 240);
        assert_eq!(p.subobs_id, "1419789248");
    }

    #[test]
    fn test_read_packet_map() {
        let filename = "test_files/1419789248_1419789248_91_small.sub";

        // Open file
        let mut file = File::open(filename).unwrap();

        // Pretend we have already read the header
        let p: PsrdadaHeader = PsrdadaHeader{map_start_index: 6351360, map_length: 150000, ninputs: 240, subobs_id: "1419789248".to_string(),  chan:"91".to_string()};
        
        // Setup buffer
        let mut packets_lost: Vec<u16> = vec![0; p.ninputs];

        // Do the actual test!
        read_packet_map(&mut file, p.ninputs, p.map_start_index, p.map_length, &mut packets_lost).unwrap();

        // Check!
        assert_eq!(packets_lost, [0,0,0,0,0,0,0,0,1,1,0,0,0,0
            ,0,0,0,0,0,0,0,0,0,0,0,0,0,0
            ,0,0,0,0,0,0,0,0,0,0,0,0,1,1
            ,1,1,0,0,0,0,1,0,0,0,0,0,0,0
            ,0,2,2,1,1,1,1,1,1,0,0,1,0,0
            ,0,0,1,0,0,0,0,0,0,0,24,24,25,25
            ,24,27,27,27,24,25,25,25,26,27,26,27,0,0
            ,1,1,0,3,3,3,0,0,1,1,0,3,5000,2
            ,0,0,0,0,0,0,0,0,0,0,0,0,0,0
            ,0,0,1,1,2,1,1,0,0,0,1,1,1,2
            ,1,1,1,1,1,0,0,0,0,0,0,0,1,1
            ,0,0,0,0,0,0,0,0,0,0,0,0,0,0
            ,0,0,0,0,0,0,0,0,0,0,0,0,0,0
            ,0,0,0,0,0,0,0,0,0,0,1,0,0,0
            ,0,0,0,0,1,1,0,0,0,0,1,1,26,25
            ,25,24,27,25,24,23,25,23,26,24,26,23,27,23
            ,25,25,27,25,26,24,27,26,27,25,28,25,24,23
            ,27,24]);
    }
}