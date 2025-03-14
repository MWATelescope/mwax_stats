// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
mod subfile;

use clap::{crate_authors, crate_description, crate_version, App, Arg};
use gethostname::gethostname;
use log::debug;
use std::{env, ffi::OsString, fmt::Debug, path::Path};

/// This is main entry point of the executable.
///
/// # Arguments
///
/// None
///
///
/// # Returns
///
/// * None
///
fn main() {
    env_logger::try_init().unwrap_or(());
    debug!("start main");
    main_with_args(env::args());
    debug!("end main");
}

/// This takes any command line arguments, processes them and takes action
///
/// # Arguments
///
/// * `args` - command line args for the executable
///
///
/// # Returns
///
/// * N/A
///
pub(crate) fn main_with_args<I, T>(args: I)
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
    I: Debug,
{
    let hostname = gethostname();
        
    debug!("args:\n{:?}", &args);

    let app = App::new("mwax_packet_stats")
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::with_name("subfile_name")
                .short("s")
                .takes_value(true)
                .required(true)
                .help("Sets the subfile name/path."),
        )
        .arg(
            Arg::with_name("output-dir")
                .short("o")
                .takes_value(true)
                .required(true)
                .help("Specify the directory to write output files to."),
        );        

    let arg_matches = app.get_matches_from(args);

    debug!("arg matches:\n{:?}", &arg_matches);

    // Collect inputs from the command line
    let subfile_name = arg_matches.value_of("subfile_name").unwrap();
    let output_dir = arg_matches.value_of("output-dir").unwrap();
    
    // Read Packet stats
    subfile::process_subfile_packet_map_data(Path::new(subfile_name), Path::new(output_dir), hostname.to_str().unwrap()).expect("Error");    

}
