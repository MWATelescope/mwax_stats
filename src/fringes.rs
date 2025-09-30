// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
use crate::processing;
use log::{debug, info, trace};
use mwalib::CorrelatorContext;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Outputs one binary file for an observation.
///
/// Each file is named OBSID_fringes_NFINECHANSchans_128T.dat (128 is the number of tiles which may vary)
///
/// File format 3 floats * num fine channels per coarse * coarse channels passed in * tiles:
/// Slowest moving -> fastest moving
/// [ant1][ant2][fine chan freq][XX phase][YY phase]
///
///     fine chan freq (MHz)
///     phase(XX) (deg)
///     phase(YY) (deg)
pub fn output_fringes(
    context: &CorrelatorContext,
    output_dir: &str,
    use_any_timestep: bool,
    max_memory_gb: Option<f32>,
    correct_cable_lengths: bool,
    correct_digital_gains: bool,
    correct_passband_gains: bool,
    correct_geometry: bool,
) {
    info!("Starting output_fringes()...");

    // Determine timestep and coarse channel range
    // For fringes we only want all the common good timesteps if possible; and one coarse channel
    let (timestep_range, coarse_chan_range) =
        processing::get_timesteps_coarse_chan_ranges(context, use_any_timestep, max_memory_gb)
            .unwrap();

    // Output the timestep and coarse channel ranges and debug
    debug!(
        "Timesteps   : {} indicies: {}..{}",
        timestep_range.len(),
        timestep_range.start,
        timestep_range.end - 1
    );
    debug!(
        "Coarse chans: {} indicies: {}..{}",
        coarse_chan_range.len(),
        coarse_chan_range.start,
        coarse_chan_range.end - 1
    );

    // Get data
    let jones_array = processing::get_corrected_data(
        context,
        &timestep_range,
        &coarse_chan_range,
        correct_cable_lengths,
        correct_digital_gains,
        correct_passband_gains,
        correct_geometry,
    );

    // Open a file for writing
    let output_filename = Path::new(output_dir).join(format!(
        "{}_fringes_{}chans_{}T_ch{}.dat",
        context.metafits_context.obs_id,
        context.metafits_context.num_corr_fine_chans_per_coarse,
        context.metafits_context.num_ants,
        context.coarse_chans[coarse_chan_range.start].rec_chan_number
    ));

    // Establish the starting index for the fine channel frequency array. It is for all channels whether we provided data or not
    let fine_chan_freq_index =
        coarse_chan_range.start * context.metafits_context.num_corr_fine_chans_per_coarse;

    // Create output file for writing
    let output_file =
        File::create(&output_filename).expect("Unable to open fringe file for writing");

    let mut writer = BufWriter::new(&output_file);

    // Loop through all of the baselines
    for (bl_index, bl) in context.metafits_context.baselines.iter().enumerate() {
        // Loop through fine channels
        for fine_chan_index in 0..context.metafits_context.num_corr_fine_chans_per_coarse {
            let mut xx_r: f64 = 0.0;
            let mut xx_i: f64 = 0.0;
            let mut yy_r: f64 = 0.0;
            let mut yy_i: f64 = 0.0;

            // Determine fine chan frequency
            let fine_chan_freq_mhz = (&context.metafits_context.metafits_fine_chan_freqs_hz
                [fine_chan_freq_index + fine_chan_index]
                / 1000000.0) as f32;

            for timestep_loop_index in 0..timestep_range.len() {
                // The Birli Jones Matrix is in order:
                // timestep, fine_chan, baseline and then pol
                let data = jones_array[[timestep_loop_index, fine_chan_index, bl_index]];

                // Calculate Phase of XX and YY
                // data for each fine channel is: xx_r, xx_i, xy_r, xy_i, yx_r, yx_i, yy_r, yy_i
                xx_r += data[0].re as f64;
                xx_i += data[0].im as f64;
                yy_r += data[3].re as f64;
                yy_i += data[3].im as f64;
            }

            let xx_phase_deg: f32 = xx_i.atan2(xx_r).to_degrees() as f32;
            let yy_phase_deg: f32 = yy_i.atan2(yy_r).to_degrees() as f32;

            if bl_index == 1 {
                trace!(
                    "{},{},{},{},{},{},{},{},{},{},{}",
                    bl.ant1_index,
                    bl.ant2_index,
                    fine_chan_freq_index + fine_chan_index,
                    fine_chan_index,
                    fine_chan_freq_mhz,
                    xx_phase_deg,
                    yy_phase_deg,
                    xx_r,
                    xx_i,
                    yy_r,
                    yy_i
                );
            }

            let float_vec = vec![fine_chan_freq_mhz, xx_phase_deg, yy_phase_deg];

            let float_bytes: Vec<u8> = floats_to_bytes(float_vec);
            // Write data to file
            writer
                .write_all(&float_bytes)
                .expect("Error writing fringe data");
        }
    }

    writer.flush().expect("Error flushing output file to disk");

    info!(
        "Done! {} written.",
        &output_filename
            .into_os_string()
            .to_str()
            .expect("Could not convert path into string")
    );
}

pub fn floats_to_bytes(floats: Vec<f32>) -> Vec<u8> {
    let mut byte_array = Vec::new();
    for f in floats {
        // Use to_le_bytes() for little-endian or to_be_bytes() for big-endian
        byte_array.extend_from_slice(&f.to_le_bytes());
    }
    byte_array
}
