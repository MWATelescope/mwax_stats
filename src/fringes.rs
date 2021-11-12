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
pub fn output_fringes(
    context: &CorrelatorContext,
    output_dir: &str,
    use_any_timestep: bool,
    correct_cable_lengths: bool,
    correct_geometry: bool,
) {
    info!("Starting output_fringes()...");

    // Determine timestep and coarse channel range
    // For fringes we only want all the common good timesteps if possible; and one coarse channel
    let (timestep_range, coarse_chan_range) =
        processing::get_timesteps_coarse_chan_ranges(&context, use_any_timestep).unwrap();

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

    // Get a jones matrix
    debug!("Generating jones array...");
    let (mut jones_array, _) =
        birli::context_to_jones_array(&context, &timestep_range, &coarse_chan_range, None);

    debug!(
        "Jones array shape (timesteps, fine_chans, baselines){:?}",
        jones_array.shape()
    );

    if correct_cable_lengths {
        debug!("Correcting cable lengths...");
        birli::corrections::correct_cable_lengths(&context, &mut jones_array, &coarse_chan_range);
    }

    if correct_geometry {
        debug!("Correcting geometry...");
        birli::corrections::correct_geometry(
            &context,
            &mut jones_array,
            &timestep_range,
            &coarse_chan_range,
            None,
        );
    }

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
    let mut output_file =
        File::create(&output_filename).expect("Unable to open fringe file for writing");

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
