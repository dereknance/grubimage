use anyhow::{anyhow, Context, Result};
use grubimage::{
    args::{BuildArgs, BuildCommand},
    builder::Builder,
    config, help,
};
use std::{
    env,
    path::{Path, PathBuf},
};

pub fn main() -> Result<()> {
    let mut raw_args = env::args();

    let executable_name = raw_args
        .next()
        .ok_or_else(|| anyhow!("no first argument (executable name)"))?;
    let file_stem = Path::new(&executable_name)
        .file_stem()
        .and_then(|s| s.to_str());
    if file_stem != Some("cargo-grubimage") {
        return Err(anyhow!(
            "Unexpected executable name: expected `cargo-grubimage`, got: `{:?}`",
            file_stem
        ));
    }
    if raw_args.next().as_deref() != Some("grubimage") {
        return Err(anyhow!("Please invoke this as `cargo grubimage`"));
    }

    match BuildCommand::parse_args(raw_args)? {
        BuildCommand::Build(args) => build(args),
        BuildCommand::Version => {
            help::print_version();
            Ok(())
        }
        BuildCommand::Help => {
            help::print_cargo_grubimage_help();
            Ok(())
        }
    }
}

fn build(args: BuildArgs) -> Result<()> {
    let mut builder = Builder::new(args.manifest_path().map(PathBuf::from))?;
    let config = config::read_config(builder.manifest_path())?;
    let quiet = args.quiet();

    let executables = builder.build_kernel(&args.cargo_args(), &config, quiet)?;
    if executables.is_empty() {
        return Err(anyhow!("no executables built"));
    }

    for executable in executables {
        let out_dir = executable
            .parent()
            .ok_or_else(|| anyhow!("executable has no parent path"))?;
        let bin_name = &executable
            .file_stem()
            .ok_or_else(|| anyhow!("executable has no file stem"))?
            .to_str()
            .ok_or_else(|| anyhow!("executable file stem not valid utf8"))?;

        let iso_files = out_dir.join("isofiles");
        // We don't have access to a CARGO_MANIFEST_DIR environment variable
        // here because `cargo grubimage` is started directly by the user. We
        // therefore have to find out the path to the Cargo.toml of the
        // executables ourselves. For workspace projects, this can be a
        // different Cargo.toml than the Cargo.toml in the current directory.
        //
        // To retrieve the correct Cargo.toml path, we look for the binary name
        // in the `cargo metadata` output and then get the manifest path from
        // the corresponding package.
        let kernel_package = builder
            .kernel_package_for_bin(bin_name)
            .context("Failed to run cargo metadata to find out kernel manifest path")?
            .ok_or_else(|| anyhow!("Failed to find kernel binary in cargo metadata output"))?;
        let kernel_manifest_path = &kernel_package.manifest_path.to_owned();

        let grubimage_path = out_dir.join(format!("grubimage-{}.iso", bin_name));

        let grubimage = grubimage::builder::Grubimage {
            kernel_manifest: &kernel_manifest_path,
            bin_path: &executable,
            output_bin_path: &grubimage_path,
            quiet,
            release: args.release(),
            iso_dir_path: &iso_files,
            bin_name: &bin_name,
        };

        builder.create_grubimage(&grubimage)?;
        if !args.quiet() {
            println!(
                "Created grubimage for `{}` at `{}`",
                bin_name,
                grubimage_path.display()
            );
        }
    }

    Ok(())
}
