extern crate file_utils;
use log::{debug, info, trace};
use mwalib::CorrelatorContext;
use core::ops::Range;
use crate::errors::MwaxStatsError;
use birli::corrections::ScrunchType;
use birli::io::read_mwalib;
use birli::ndarray::{ArrayBase, Dim, OwnedRepr};
use birli::passband_gains::PFB_JAKE_2022_200HZ;
use birli::{get_weight_factor, Jones};
use birli::VisSelection;

pub fn print_info(context: &CorrelatorContext) {
    trace!("{}", context);
    info!("Observation             : {}", context.metafits_context.obs_id);
    info!("Fine chans per coarse   : {}", context.metafits_context.num_corr_fine_chans_per_coarse);
    info!("Common Good Timesteps   : {:?}", context.common_good_timestep_indices);
    info!("Common Good Coarse chans: {:?}", context.common_good_coarse_chan_indices);
    info!("Common Timesteps        : {:?}", context.common_timestep_indices);
    info!("Common Coarse chans     : {:?}", context.common_coarse_chan_indices);
}

/// Get a range of timesteps/coarse channels
/// Returns a result containing a Range of timestep indices and a Range of Coarse channel indices
/// Will preferably try to get the common good timesteps/coarse channels. If use_any_timesteps is True it
/// will defer to common timesteps/coarse channels if no common good exist.
pub fn get_timesteps_coarse_chan_ranges(context: &CorrelatorContext, use_any_timestep: bool) -> Result<(Range<usize>, Range<usize>), MwaxStatsError> {
    if context.num_common_good_timesteps > 0 {
        Ok(((*context.common_good_timestep_indices.first().unwrap()..context.common_good_timestep_indices.last().unwrap() + 1), (*context.common_good_coarse_chan_indices.first().unwrap()..context.common_good_coarse_chan_indices.iter().last().unwrap() + 1)))
    } else if use_any_timestep {
        if context.num_common_timesteps > 0 {
            Ok(((*context.common_timestep_indices.first().unwrap()..context.common_timestep_indices.last().unwrap() + 1), (*context.common_coarse_chan_indices.first().unwrap()..context.common_coarse_chan_indices.iter().last().unwrap() + 1)))
        }
        else {
            Err(MwaxStatsError::NoCommonGoodTimestepCCFound)
        }
    } else {
        Err(MwaxStatsError::NoCommonTimestepCCFound)
    }    
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

    // Allocate jones array
    let mut jones_array = vis_sel.allocate_jones(fine_chans_per_coarse).unwrap();

    // Allocate flags array
    let mut flag_array = vis_sel.allocate_flags(fine_chans_per_coarse).unwrap();

    // Allocate weights array
    let mut weight_array = vis_sel.allocate_weights(fine_chans_per_coarse).unwrap();
    weight_array.fill(get_weight_factor(context) as _);

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

    if correct_cable_lengths {
        debug!("Correcting cable lengths...");
        birli::corrections::correct_cable_lengths(
            context,
            jones_array.view_mut(),
            coarse_chan_range,
            false,
        );
    }

    if correct_digital_gains {
        debug!("Correcting digital gains...");
        let sel_ant_pairs = vis_sel.get_ant_pairs(&context.metafits_context);
        birli::corrections::correct_digital_gains(
            context,
            jones_array.view_mut(),
            coarse_chan_range,
            &sel_ant_pairs,
        )
        .unwrap();
    }

    if correct_passband_gains {
        debug!("Correcting coarse passband gains...");
        birli::corrections::correct_coarse_passband_gains(
            jones_array.view_mut(),
            weight_array.view_mut(),
            PFB_JAKE_2022_200HZ,
            fine_chans_per_coarse,
            &ScrunchType::from_mwa_version(context.metafits_context.mwa_version.unwrap()).unwrap(),
        )
        .unwrap();
    }

    if correct_geometry {
        debug!("Correcting geometry...");
        birli::corrections::correct_geometry(
            context,
            jones_array.view_mut(),
            timestep_range,
            coarse_chan_range,
            None,
            None,
            false,
        );
    }
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
        let result1 = get_timesteps_coarse_chan_ranges(&context,false);
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
        let result = get_timesteps_coarse_chan_ranges(&context,true);
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
}