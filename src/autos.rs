// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
use crate::processing;
use file_utils::write::Write;
use log::{debug, info, trace};
use mwalib::CorrelatorContext;
use std::fs::File;
use std::path::Path;

/// Outputs one binary file for an observation.
///
/// Each file is named OBSID_autos_FINECHANSchans_128T.dat  (128 is the number of tiles which may vary)
///
/// File format 3 floats * num fine channels per coarse * coarse channels passed in * tiles:
/// Slowest moving -> fastest moving
/// [ant][fine chan freq][XX][YY]
///
///     fine chan freq (MHz)
///     XX pow (dB)
///     YY pow (dB)
pub fn output_autocorrelations(context: &CorrelatorContext, output_dir: &str) {
    info!("Starting output_autocorrelations()...");

    // Get data info a buffer
    let data: Vec<f32> = processing::get_data(context);

    // Open a file for writing
    let output_filename = Path::new(output_dir).join(format!(
        "{}_autos_{}chans_{}T.dat",
        context.metafits_context.obs_id,
        context.metafits_context.num_corr_fine_chans_per_coarse,
        context.metafits_context.num_ants
    ));

    let mut output_file =
        File::create(&output_filename).expect("Unable to open autos file for writing");

    // Loop through all of the baselines
    for (bl_index, bl) in context.metafits_context.baselines.iter().enumerate() {
        let ant = &context.metafits_context.antennas[bl.ant1_index];

        // We only care about auto correlations
        if bl.ant1_index == bl.ant2_index {
            debug!(
                "Antenna index: {} TileID: {} ({})",
                bl.ant1_index, ant.tile_id, ant.tile_name
            );

            // Loop through all coarse channels
            for (loop_index, cgcc_index) in
                context.common_good_coarse_chan_indices.iter().enumerate()
            {
                let chan = &context.coarse_chans[*cgcc_index];

                debug!(
                    "Coarse chan processed: {} index: {} (rec: {})",
                    loop_index, *cgcc_index, chan.rec_chan_number
                );

                // Establish the starting index for the fine channel frequency array. It is for all channels whether we provided data or not
                let fine_chan_freq_index =
                    *cgcc_index * context.metafits_context.num_corr_fine_chans_per_coarse;

                // Establish the index to this baseline in the data vector
                // The data vector can have N coarse channels so we need to move along by that many floats
                let mut data_index: usize = (loop_index * &context.num_timestep_coarse_chan_floats)
                    + bl_index
                        * (context.metafits_context.num_corr_fine_chans_per_coarse
                            * context.metafits_context.num_visibility_pols
                            * 2);

                // Loop through fine channels
                for fine_chan in 0..context.metafits_context.num_corr_fine_chans_per_coarse {
                    // Calculate Power in X and Y
                    // data for each fine channel is: xx_r, xx_i, xy_r, xy_i, yx_r, yx_i, yy_r, yy_i
                    let xx_r = data[data_index];
                    let yy_r = data[data_index + 6];
                    let xx_pow: f32 = 10.0 * f32::log10(xx_r + 1.0);
                    let yy_pow: f32 = 10.0 * f32::log10(yy_r + 1.0);

                    // Determine fine chan frequency
                    let fine_chan_freq_mhz = (&context.metafits_context.metafits_fine_chan_freqs_hz
                        [fine_chan_freq_index + fine_chan]
                        / 1000000.0) as f32;

                    trace!(
                        "ant: {} fine_chan_freq_index {} finech: {} freq: {} MHz xx_r: {} yy_r: {} xx_pow: {} yy_pow: {}",
                        bl.ant1_index, fine_chan_freq_index, fine_chan, fine_chan_freq_mhz, xx_r, yy_r, xx_pow, yy_pow
                    );

                    // Write data to file
                    output_file
                        .write_f32(fine_chan_freq_mhz)
                        .expect("Error writing fine_chan_freq_MHz data");
                    output_file
                        .write_f32(xx_pow)
                        .expect("Error writing xx_pow data");
                    output_file
                        .write_f32(yy_pow)
                        .expect("Error writing yy_pow data");

                    // Determine index of next data
                    // [bl][ch][pol][r/i]
                    // increment from the start of the baseline along the fine channels
                    // Each fine channel has 4 pols and 2 values
                    data_index += context.metafits_context.num_visibility_pols * 2;
                }
            }
        }
    }

    info!(
        "Done! {} written.",
        &output_filename
            .into_os_string()
            .to_str()
            .expect("Could not convert path into string")
    );
}
