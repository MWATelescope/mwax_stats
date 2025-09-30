// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

extern crate file_utils;
use log::{debug, info, trace};
use ndarray::{ArrayBase, Dim, OwnedRepr};
use core::ops::Range;
use crate::errors::MwaxStatsError;
use birli::{
    flag_to_weight_array, flags::get_weight_factor, io::read_mwalib, marlu::{
        constants::{
            MWA_HEIGHT_M, MWA_LAT_RAD, MWA_LONG_RAD,
        },
        mwalib::CorrelatorContext,
        LatLngHeight, RADec,
    }, FlagContext, Jones, PreprocessContext, VisSelection
};

pub fn print_info(context: &CorrelatorContext) {
    trace!("{}", context);
    info!("Observation             : {}", context.metafits_context.obs_id);
    info!("Fine chans per coarse   : {}", context.metafits_context.num_corr_fine_chans_per_coarse);
    info!("Common Good Timesteps   : {:?}", context.common_good_timestep_indices);
    info!("Common Good Coarse chans: {:?}", context.common_good_coarse_chan_indices);
    info!("Common Timesteps        : {:?}", context.common_timestep_indices);
    info!("Common Coarse chans     : {:?}", context.common_coarse_chan_indices);
}

pub fn bytes_to_gigabytes(bytes_value: usize) -> f32 {
    bytes_value as f32 / (1000.0 * 1000.0 * 1000.0)
}

pub fn gigabytes_to_bytes(gigabytes_value: f32) -> usize {
    (gigabytes_value * (1000.0 * 1000.0 * 1000.0)) as usize
}

/// Get a range of timesteps/coarse channels
/// Returns a result containing a Range of timestep indices and a Range of Coarse channel indices
/// Will preferably try to get the common good timesteps/coarse channels. If use_any_timesteps is True it
/// will defer to common timesteps/coarse channels if no common good exist.
/// We can limit the memory used too (especially good for testing on a laptop)
pub fn get_timesteps_coarse_chan_ranges(context: &CorrelatorContext, use_any_timestep: bool, memory_limit_gb: Option<f32>) -> Result<(Range<usize>, Range<usize>), MwaxStatsError> {
    // Get as many good/common timesteps that can fit into our memory limit    
    let mut returned_timesteps = if context.num_common_good_timesteps > 0 {
        *context.common_good_timestep_indices.first().unwrap()..context.common_good_timestep_indices.last().unwrap() + 1
    } else if use_any_timestep {
        if context.num_common_timesteps > 0 {
            *context.common_timestep_indices.first().unwrap()..context.common_timestep_indices.last().unwrap() + 1
        }
        else {
             return Err(MwaxStatsError::NoCommonGoodTimestepCCFound)
        }
    } else {
        return Err(MwaxStatsError::NoCommonTimestepCCFound)
    };
    debug!("{} Timesteps [{}:{}] selected",returned_timesteps.len(), returned_timesteps.start, returned_timesteps.end);
    
    // We pretty much always want as many coarse channels as possible
    let returned_coarse_chans = if context.num_common_good_timesteps > 0 {
        *context.common_good_coarse_chan_indices.first().unwrap()..context.common_good_coarse_chan_indices.iter().last().unwrap() + 1
    } else if use_any_timestep {
        if context.num_common_timesteps > 0 {
            *context.common_coarse_chan_indices.first().unwrap()..context.common_coarse_chan_indices.iter().last().unwrap() + 1
        }
        else {
             return Err(MwaxStatsError::NoCommonGoodTimestepCCFound)
        }
    } else {
        return Err(MwaxStatsError::NoCommonTimestepCCFound)
    };
    debug!("{} Coarse channels: [{}:{}] selected",returned_coarse_chans.len(), returned_coarse_chans.start, returned_coarse_chans.end);

    // Determine the number of timesteps we can fit into memory    
    if memory_limit_gb.is_some() {
        let memory_limit_bytes: usize = gigabytes_to_bytes(memory_limit_gb.unwrap());
        let ts_bytes = context.num_timestep_coarse_chan_bytes * returned_coarse_chans.len();
        let mwax_num_ts_in_memory: usize = memory_limit_bytes / ts_bytes;

        debug!("Data selection will use {} GB of memory. Memory limit is {} GB. Number of timesteps that can fit in memory: {}.", bytes_to_gigabytes(ts_bytes * returned_timesteps.len()), memory_limit_gb.unwrap(), mwax_num_ts_in_memory);

        if returned_timesteps.len() > mwax_num_ts_in_memory {
            // Reduce the number of timesteps        
            returned_timesteps.end = returned_timesteps.end - (returned_timesteps.len() - mwax_num_ts_in_memory);

            debug!("Selected timesteps would have exceeded memory limit.");
            debug!("Reducing timesteps to {} Timesteps [{}:{}] ({} GB)", returned_timesteps.len(), returned_timesteps.start, returned_timesteps.end, (returned_timesteps.len() as f32 * bytes_to_gigabytes(ts_bytes)));
        }
    }

    Ok((returned_timesteps, returned_coarse_chans))
}

///
/// Given a CorrelatorContext and timestep and coarse channel range, along with correction flags, performs the corrections on the data and returns a Jones matrix
///
pub fn get_corrected_data(
    context: &CorrelatorContext,
    timestep_range: &Range<usize>,
    coarse_chan_range: &Range<usize>,
    correct_cable_lengths: bool,
    correct_digital_gains: bool,
    correct_passband_gains: bool,
    correct_geometry: bool,    
) -> ArrayBase<OwnedRepr<Jones<f32>>, Dim<[usize; 3]>> {
    info!("Correcting data for {} timesteps and {} coarse channels",timestep_range.len(),  coarse_chan_range.len());

    // Determine which timesteps and coarse channels we want to use
    let mut vis_sel = VisSelection::from_mwalib(context).unwrap();

    // Override the timesteps because we only want our single timestep
    vis_sel.timestep_range = timestep_range.clone();

    // Get number of fine chans
    let fine_chans_per_coarse = context.metafits_context.num_corr_fine_chans_per_coarse;

    // Setup flag context
    let flag_ctx = FlagContext::from_mwalib(context);

    // Allocate flags array
    let mut flag_array = vis_sel.allocate_flags(fine_chans_per_coarse).unwrap();

    // Set the flags
    flag_ctx
        .set_flags(
            flag_array.view_mut(),
            &vis_sel.timestep_range,
            &vis_sel.coarse_chan_range,
            &vis_sel.get_ant_pairs(&context.metafits_context),
        )
        .unwrap();

    // Allocate jones array
    let mut jones_array = vis_sel.allocate_jones(fine_chans_per_coarse).unwrap();
    
    // read visibilities out of the gpubox files
    info!("Reading visibilities");
    read_mwalib(
        &vis_sel,
        context,
        jones_array.view_mut(),
        flag_array.view_mut(),
        false,
    )
    .unwrap();

    debug!(
        "Jones array shape (timesteps, fine_chans, baselines){:?}",
        jones_array.shape()
    );

    // Allocate weights array    
    let weight_factor = get_weight_factor(context);
    let mut weight_array = flag_to_weight_array(flag_array.view(), weight_factor);

    let prep_ctx = PreprocessContext {
        //Uncomment this is we need to do flagging, but I don't think we do
        //aoflagger_strategy: match perform_rfi_flagging { true => Some("/usr/share/aoflagger/strategies/mwa-default.lua".to_string()), false=>None} ,
        array_pos: LatLngHeight {
            longitude_rad: MWA_LONG_RAD,
            latitude_rad: MWA_LAT_RAD,
            height_metres: MWA_HEIGHT_M,
        },
        phase_centre: RADec::from_mwalib_phase_or_pointing(&context.metafits_context),
        correct_van_vleck: false,
        correct_cable_lengths,
        correct_digital_gains,
        correct_geometry,
        draw_progress: false,
        passband_gains: match correct_passband_gains {
            true => {
                        match context.metafits_context.oversampled {
                            true => Some(birli::passband_gains::OSPFB_JAKE_2025_200HZ),
                            _ => Some(birli::passband_gains::PFB_JAKE_2022_200HZ)
                        }
                    },
            _ => None
        },
        calsols: None,        
    };

    prep_ctx
        .preprocess(
            context,
            jones_array.view_mut(),
            weight_array.view_mut(),
            flag_array.view_mut(),
            &vis_sel,
        )
        .unwrap();

    info!("Corrections complete");
    
    jones_array
}

/// Given a correlator context, read the timestep of the coarse channel provided.
pub fn get_data(
    context: &CorrelatorContext,
    timestep_index: usize,
    coarse_chan_index: usize,
) -> Vec<f32> {
    // Get the data for the timestep and coarse channel passed in
    info!(
        "Reading data from timestep index: {} GPS Time: {} / coarse channel index: {} rec_chan: {}...",
        timestep_index, 
        context.timesteps[timestep_index].gps_time_ms as f64 / 1000.,
        coarse_chan_index,
        context.coarse_chans[coarse_chan_index].rec_chan_number
    );
    
    let mut data: Vec<f32> = vec![
        0.;
        context.num_timestep_coarse_chan_floats
    ];

    debug!(
        "Buffer of {} length created ({} floats).",
        data.len(),        
        &context.num_timestep_coarse_chan_floats
    );

    context
        .read_by_baseline_into_buffer(
            timestep_index,
            coarse_chan_index,
            &mut data,
        )
        .expect("Failed to read data by baseline into buffer");
    debug!(
        "{} bytes read for coarse channel {}",
        &context.num_timestep_coarse_chan_bytes, coarse_chan_index
    );

    data
}

#[cfg(test)]
mod tests {
    use birli::CorrelatorContext;

    use crate::processing::{bytes_to_gigabytes, gigabytes_to_bytes};

    use super::get_timesteps_coarse_chan_ranges;

    const TEST_METAFITS_FILENAME: &str = "test_files/1244973688_1_timestep/1244973688.metafits";
    const TEST_MWAX_FITS_FILENAME: &str = "test_files/1244973688_1_timestep/1244973688_20190619100110_ch114_000.fits";

    fn get_context() -> Result<CorrelatorContext, mwalib::MwalibError> {
        let filenames = vec![TEST_MWAX_FITS_FILENAME];
        CorrelatorContext::new(TEST_METAFITS_FILENAME,  &filenames)
    }

    #[test]
    fn test_get_timesteps_coarse_chan_ranges_no_common_good() {
        let context_result = get_context();

        // Test # 1 is it ok?
        assert!(context_result.is_ok());

        // unwrap the context
        let context = context_result.unwrap();

        // Now get the ts anc cc ranges- passing use_any_timestep = False
        // The example fits file only has 1 timestep and is within the quaktime, so this should fail
        // as there will be no common good timesteps
        let result1 = get_timesteps_coarse_chan_ranges(&context,false, None);
        assert!(result1.is_err());        
    }

    #[test]
    fn test_get_timesteps_coarse_chan_ranges_good() {
        let context_result = get_context();

        // Test # 1 is it ok?
        assert!(context_result.is_ok());

        // unwrap the context
        let context = context_result.unwrap();        

        // Now get the ts anc cc ranges- passing use_any_timestep = True
        // The example fits file only has 1 timestep and is within the quaktime, so this should succeed as we've said to use any (common) timestep        
        let result = get_timesteps_coarse_chan_ranges(&context,true, None);
        assert!(result.is_ok());
        let (ts_range, cc_range) = result.unwrap();

        // Check the ranges we got back
        assert_eq!(ts_range.len(), 1);
        assert_eq!(ts_range.start, 0);
        assert_eq!(ts_range.end, 1);

        assert_eq!(cc_range.len(), 1);
        assert_eq!(cc_range.start, 10);
        assert_eq!(cc_range.end, 11);
    }

    #[test]
    fn test_bytes_to_gigabytes() {
        assert_eq!(10.0, bytes_to_gigabytes(10_000_000_000));
    }

    #[test]
    fn test_gigabytes_to_bytes() {
        assert_eq!(10_000_000_000, gigabytes_to_bytes(10.0));
    }
}