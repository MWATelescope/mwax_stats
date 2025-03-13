use std::path::Path;
use std::fs::File;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::str;
use anyhow::{anyhow, Result};
use ndarray::{Array,Array1};
use log::{debug, info};
use rayon::prelude::*;

const PSRDADA_HEADER_LEN:usize = 4096;
const KEY_SUBOBS_ID: &str = "SUBOBS_ID";
const KEY_IDX_PACKET_MAP: &str = "IDX_PACKET_MAP";
const KEY_NINPUTS: &str = "NINPUTS";
const KEY_COARSE_CHANNEL: &str = "COARSE_CHANNEL";

pub(crate) fn get_subfile_packet_map_data(subfile_name: &Path, output_dir: &Path, hostname: &str) -> Result<(), anyhow::Error> {
    // Open the file    
    let mut file = File::open(subfile_name)?;
    
    // Read header and get the packet stats indices
    let packet_stats_idx = read_subfile_header_key(&mut file, KEY_IDX_PACKET_MAP)?;
    let ninputs: usize = read_subfile_header_key(&mut file, KEY_NINPUTS)?.parse()?;
    let subobs_id = read_subfile_header_key(&mut file, KEY_SUBOBS_ID)?;
    let chan = read_subfile_header_key(&mut file, KEY_COARSE_CHANNEL)?;
    
    // IDX_PACKET_MAP contains X+Y where X is the start byte of Block0 and Y is the length
    let (start, length) = packet_stats_idx.split_once("+").unwrap_or(("",""));

    // Parse the start and length to usize
    let map_start_index:u64 = start.parse()?;
    let map_length:usize = length.parse()?;

    // Allocate a buffer
    let mut buf = vec![0_u8; map_length];

    // Read the data from Block0 of the file
    // We should already be at the start of Block0 from reading the header, but seek to start of packet map
    file.seek(SeekFrom::Start(PSRDADA_HEADER_LEN as u64 + map_start_index))?;
    file.read_exact(&mut buf)?;
    debug!("{:?}", buf);

    // Create identity array/bit lookup
    let mut ident_array: Array1<u16> = Array::zeros(256);
    // build an identity array, just the values [0..255]
    for i in 0..256 {
        ident_array[i] = i as u16;
    }
    // for each bitpair, adds the high bit to the low bit, resulting in four counts each in the range [0..2]
    ident_array = ((&ident_array & 0xAA) >> 1) + (&ident_array & 0x55); 
    // for each nybble, sum the bitpairs
    ident_array = ((&ident_array & 0xCC) >> 2) + (&ident_array & 0x33);
    // sum the nybbles    
    let count_bits: Array1<u16> = ((&ident_array & 0xF0) >> 4) + (&ident_array & 0x0F);

    // Determine max number of packets
    let num_bytes_per_input = buf.len() / ninputs;
        
    // Change packet_map into a 2d array but rf_input
    let packet_map = Array::from_shape_vec((ninputs, num_bytes_per_input), buf)?;    

    // Create a buffer for the counts
    let mut packets_lost: Vec<u16> = vec!(0; ninputs);
    
    // Loop through each input in parallel
    packets_lost.par_iter_mut().enumerate().for_each(| (input, v)| {                
        // For each input we have a whole bunch of byte sized bitmaps e.g. 0001000,11110111,etc
        // count bits provides a lookup which converts the byte value into a count of bits
        // e.g. a value of 00000000 has 0 bits. A value of 00000001 has 1 bit, a value of 01010101 has 4 bits.        
        for b in 0..num_bytes_per_input {
            // Accumulate the count of bits. Subtract 8 as for each byte we want the number of packets LOST
            // i.e. inverse of packets CAPTURED.
            *v += 8 - count_bits[packet_map[[input, b]] as usize];
        }        
        debug!("i={}, lost={}", input, v);
    });

    // Now write the data file
    let output_filename = output_dir.join(format!("packetstats_{}_{}T_ch{}_{}.dat", subobs_id, ninputs/2, chan, hostname));
    let out_file = File::create(output_filename)?;

    out_file.write( packets_lost.to_le_bytes())?;

    Ok(())
}

fn read_subfile_header_key(file: &mut File, key: &str) -> Result<String, anyhow::Error> {            
    // Read into byte buffer
    let mut buf = [0_u8; PSRDADA_HEADER_LEN];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut buf)?;
    
    // Convert the bytes into a UTF-8 string
    let contents: Vec<&str> = str::from_utf8(&buf)?.split("\n").collect();

    // Split line into key<space>value
    // If key matches, return it and the value
    // Otherwise keep looking
    for line in contents {        
        let (found_key, value) = line.split_once(" ").unwrap_or(("",""));

        if found_key == key {
            info!("Read {}={} from {:?}", key, value, file);
            return Ok(value.to_string());
        }
    }

    // If we get here we did not find the key
    Err(anyhow!("failed to find key {} in {:?}", key, file))
}