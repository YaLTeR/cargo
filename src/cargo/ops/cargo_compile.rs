/**
 * Cargo compile currently does the following steps:
 *
 * All configurations are already injected as environment variables via the main cargo command
 *
 * 1. Read the manifest
 * 2. Shell out to `cargo-resolve` with a list of dependencies and sources as stdin
 *    a. Shell out to `--do update` and `--do list` for each source
 *    b. Resolve dependencies and return a list of name/version/source
 * 3. Shell out to `--do download` for each source
 * 4. Shell out to `--do get` for each source, and build up the list of paths to pass to rustc -L
 * 5. Call `cargo-rustc` with the results of the resolver zipped together with the results of the `get`
 *    a. Topologically sort the dependencies
 *    b. Compile each dependency in order, passing in the -L's pointing at each previously compiled dependency
 */

use std::os;
use util::config;
use util::config::{ConfigValue};
use core::{PackageSet,Source};
use core::resolver::resolve;
use sources::path::PathSource;
use ops;
use util::{other_error, CargoResult, Wrap};

// TODO: manifest_path should be Path
pub fn compile(manifest_path: &str) -> CargoResult<()> {
    log!(4, "compile; manifest-path={}", manifest_path);

    let manifest = try!(ops::read_manifest(manifest_path));

    debug!("loaded manifest; manifest={}", manifest);

    let configs = try!(config::all_configs(os::getcwd()));

    debug!("loaded config; configs={}", configs);

    let config_paths = configs.find_equiv(&"paths").map(|v| v.clone()).unwrap_or_else(|| ConfigValue::new());

    let mut paths: Vec<Path> = match config_paths.get_value() {
        &config::String(_) => return Err(other_error("The path was configured as a String instead of a List")),
        &config::List(ref list) => list.iter().map(|path| Path::new(path.as_slice())).collect()
    };

    paths.push(Path::new(manifest_path).dir_path());

    let source = PathSource::new(paths);
    let summaries = try!(source.list().wrap("unable to list packages from source"));
    let resolved = try!(resolve(manifest.get_dependencies(), &summaries).wrap("unable to resolve dependencies"));

    try!(source.download(resolved.as_slice()).wrap("unable to download packages"));

    let packages = try!(source.get(resolved.as_slice()).wrap("unable to get packages from source"));

    let package_set = PackageSet::new(packages.as_slice());

    try!(ops::compile_packages(&manifest, &package_set));

    Ok(())
}