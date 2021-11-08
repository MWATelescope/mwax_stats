// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
mod autos;
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
fn main_with_args<I, T>(args: I)
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
        .arg(Arg::with_name("fits-files").required(true).multiple(true));

    let arg_matches = app.get_matches_from(args);

    debug!("arg matches:\n{:?}", &arg_matches);

    // Collect inputs from the command line
    let metafits_filename = arg_matches.value_of("metafits").unwrap();
    let output_dir = arg_matches.value_of("output-dir").unwrap();
    let fits_files: Vec<&str> = arg_matches.values_of("fits-files").unwrap().collect();

    // Create correlator context
    let context = CorrelatorContext::new(&metafits_filename, &fits_files)
        .expect("Failed to create CorrelatoContext");

    processing::print_info(&context);

    autos::output_autocorrelations(&context, output_dir);
    fringes::output_fringes(&context, output_dir);
}
