extern crate file_utils;
use log::{debug, info};
use mwalib::{CorrelatorContext, TimeStep};

pub fn print_info(context: &CorrelatorContext) {
    debug!("{}", context);
    info!("Observation: {}", context.metafits_context.obs_id);
}

/// Given a correlator context, read the last common good timestep of all coarse channels provided.
pub fn get_data(context: &CorrelatorContext) -> Vec<f32> {
    // Get the data for the timestep for all coarse channels passed in
    info!(
        "Reading data from {} coarse channels...",
        &context.common_good_coarse_chan_indices.len()
    );

    // Determine timestep
    let t_index = context
        .common_good_timestep_indices
        .last()
        .expect("There are no common good timesteps in the observation");
    let ts: &TimeStep = &context.timesteps[*t_index];
    debug!(
        "Last Common Good Timestep: index: {} GPS time: {}",
        t_index,
        ts.gps_time_ms as f64 / 1000.0
    );

    let mut data: Vec<f32> = vec![
        0.;
        &context.num_timestep_coarse_chan_floats
            * &context.common_good_coarse_chan_indices.len()
    ];

    debug!(
        "Buffer of {} length created ({} * {} floats).",
        data.len(),
        &context.common_good_coarse_chan_indices.len(),
        &context.num_timestep_coarse_chan_floats
    );

    for (loop_index, cgcc_index) in context.common_good_coarse_chan_indices.iter().enumerate() {
        context
            .read_by_baseline_into_buffer(
                *t_index,
                *cgcc_index,
                &mut data[loop_index * &context.num_timestep_coarse_chan_floats
                    ..((loop_index + 1) * &context.num_timestep_coarse_chan_floats)],
            )
            .expect("Failed to read data by baseline into buffer");
        debug!(
            "{} bytes read for coarse channel {}",
            &context.num_timestep_coarse_chan_bytes, cgcc_index
        );
    }

    return data;
}
