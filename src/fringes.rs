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
/// Each file is named OBSID_fringes_NFINECHANSchans_128T.dat (128 is the number of tiles which may vary)
///
/// File format 3 floats * num fine channels per coarse * coarse channels passed in * tiles:
/// Slowest moving -> fastest moving
/// [ant1][ant2][fine chan freq][XX phase][YY phase]
///
///     fine chan freq (MHz)
///     phase(XX) (deg)
///     phase(YY) (deg)
pub fn output_fringes(context: &CorrelatorContext, output_dir: &str) {
    info!("Starting output_fringes()...");

    // Get data info a buffer
    let data: Vec<f32> = processing::get_data(context);

    // Open a file for writing
    let output_filename = Path::new(output_dir).join(format!(
        "{}_fringes_{}chans_{}T.dat",
        context.metafits_context.obs_id,
        context.metafits_context.num_corr_fine_chans_per_coarse,
        context.metafits_context.num_ants
    ));

    let mut output_file =
        File::create(&output_filename).expect("Unable to open fringe file for writing");

    // Loop through all of the baselines
    for (bl_index, bl) in context.metafits_context.baselines.iter().enumerate() {
        let ant = &context.metafits_context.antennas[bl.ant1_index];

        debug!(
            "Antenna index: {} TileID: {} ({})",
            bl.ant1_index, ant.tile_id, ant.tile_name
        );

        // Loop through all coarse channels
        for (loop_index, cgcc_index) in context.common_good_coarse_chan_indices.iter().enumerate() {
            let chan = &context.coarse_chans[*cgcc_index];

            trace!(
                "Coarse chan: {} (rec: {})",
                *cgcc_index,
                chan.rec_chan_number
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
                // Calculate Phase of XX and YY
                // data for each fine channel is: xx_r, xx_i, xy_r, xy_i, yx_r, yx_i, yy_r, yy_i
                let xx_r = data[data_index];
                let xx_i = data[data_index + 1];
                let yy_r = data[data_index + 6];
                let yy_i = data[data_index + 7];
                let xx_phase_deg: f32 = xx_i.atan2(xx_r).to_degrees();
                let yy_phase_deg: f32 = yy_i.atan2(yy_r).to_degrees();

                // Determine fine chan frequency
                let fine_chan_freq_mhz = (&context.metafits_context.metafits_fine_chan_freqs_hz
                    [fine_chan_freq_index + fine_chan]
                    / 1000000.0) as f32;

                trace!(
                    "ant: {},{} fine_chan_freq_index {} finech: {} freq: {} MHz xx_r: {} xx_i: {} yy_r: {} yy_i: {} xx_phase: {} yy_phase: {}",
                    bl.ant1_index, bl.ant2_index, fine_chan_freq_index + fine_chan, fine_chan, fine_chan_freq_mhz, xx_r, xx_i, yy_r, yy_i, xx_phase_deg, yy_phase_deg
                    );

                // Write data to file
                output_file
                    .write_f32(fine_chan_freq_mhz)
                    .expect("Error writing fine_chan_freq_MHz data");
                output_file
                    .write_f32(xx_phase_deg)
                    .expect("Error writing xx_phase data");
                output_file
                    .write_f32(yy_phase_deg)
                    .expect("Error writing yy_phase data");

                // Determine index of next data
                // [bl][ch][pol][r/i]
                // increment from the start of the baseline along the fine channels
                // Each fine channel has 4 pols and 2 values
                data_index += context.metafits_context.num_visibility_pols * 2;
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
