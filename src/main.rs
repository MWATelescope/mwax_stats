// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
mod autos;
mod errors;
mod fringes;
mod processing;

use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg};
use log::{debug, info};
use mwalib::CorrelatorContext;
use std::{env, ffi::OsString, fmt::Debug};

/// This is main entry point of the executable.
///
/// # Arguments
///
/// None
///
///
/// # Returns
///
/// * 0 on success, non-zero on failure
///
fn main() {
    env_logger::try_init().unwrap_or(());
    info!("start main");
    main_with_args(env::args());
    info!("end main");
}

/// This takes any command line arguments, processes them and takes action
///
/// # Arguments
///
/// None
///
///
/// # Returns
///
/// * 0 on success, non-zero on failure
///
pub(crate) fn main_with_args<I, T>(args: I)
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
    I: Debug,
{
    debug!("args:\n{:?}", &args);

    let app = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::with_name("metafits")
                .short("m")
                .takes_value(true)
                .required(true)
                .help("Sets the metafits file."),
        )
        .arg(
            Arg::with_name("output-dir")
                .short("o")
                .takes_value(true)
                .required(true)
                .help("Specify the directory to write output files to."),
        )
        .arg(
            Arg::with_name("use-any-timestep")
                .short("t")
                .takes_value(false)
                .required(false)
                .help("Use any timestep if no good (post quaktime) timestep can be found."),
        )
        .arg(Arg::with_name("fits-files").required(true).multiple(true));

    let arg_matches = app.get_matches_from(args);

    debug!("arg matches:\n{:?}", &arg_matches);

    // Collect inputs from the command line
    let metafits_filename = arg_matches.value_of("metafits").unwrap();
    let output_dir = arg_matches.value_of("output-dir").unwrap();
    let use_any_timestep: bool = arg_matches.is_present("use-any-timestep");
    let fits_files: Vec<&str> = arg_matches.values_of("fits-files").unwrap().collect();

    // Although the command line args support it, and so does `processing::get_data()` we really want to only have 1 coarse channel of data passed in
    // at this stage. So lets check for it and fail if we get >1 channel
    if fits_files.len() == 1 {
        // Create correlator context
        let context = CorrelatorContext::new(&metafits_filename, &fits_files)
            .expect("Failed to create CorrelatoContext");

        // Always print the obs info
        processing::print_info(&context);

        // Always produce autocorrelations
        autos::output_autocorrelations(&context, output_dir, use_any_timestep);

        // Only produce fringes for calibrator observations (unless we are running in debug)
        if context.metafits_context.calibrator {
            let correct_cable_lengths = !context.metafits_context.cable_delays_applied;
            let correct_geometry: bool = context.metafits_context.geometric_delays_applied
                == mwalib::GeometricDelaysApplied::No;

            info!("Correcting for cable lengths: {}.", correct_cable_lengths);
            info!("Correcting for geometry     : {}.", correct_geometry);

            fringes::output_fringes(
                &context,
                output_dir,
                use_any_timestep,
                correct_cable_lengths,
                correct_geometry,
            );
        } else {
            info!("Skipping output_fringes() as this is not a calibrator observation.");
        }
    } else {
        print!("mwax_stats currently only supports a single coarse channel of data. Exiting...")
    }
}
