extern crate clap;
extern crate dirs;
use clap::*;
use std::path;
use gettextrs::*;

pub struct CommandlineParameters {
    pub config_file : path::PathBuf,
    pub plugin_folder : path::PathBuf,
    pub print_sample_config : bool
}

/// Gets the config and plugin paths. Either from command line or from hardcoded defaults.
pub fn parse_commandline() -> CommandlineParameters {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(&*gettext("A simple status text app, inspired by i3status"))
        .help_heading(&*gettext("Arguments"))
        .arg(
            Arg::new("config")
            .short('c')
            .long("config")
            .value_name(&*gettext("FILE"))
            .about(&*gettext("Path to the configuration file"))
            .takes_value(true))
        .arg(
            Arg::new("plugins")
            .short('p')
            .long("plugins")
            .value_name(&*gettext("FOLDER"))
            .about(&*gettext("Directory from which the plugins should be loaded"))
            .takes_value(true))
        .arg(
            Arg::new("sampleconfig")
            .long("print-sample-config")
            .about(&*gettext("Prints a sample config file. Beware that the contents of the sample file depend on the loaded plugins, so don't forget to supply the plugins parameter as needed."))
            .takes_value(false))
        .after_help(&*gettext!("If no config path is given, the code looks for the \"swaystatus/config\" file in your XDG config folder (typically \"$HOME/.config/\"). If that lookup fails, loading of \"/etc/swaystatus/config\" is attempted. Similarly, if no plugin folder is given, first the existence of a folder named \"$HOME/.local/lib/swaystatus\" is checked. If this folder does not exist, a default path set at compile time is used, which in your case is \"{}\"." , get_hardcoded_default_library_path()))
        .help_template(&*gettext("\
{before-help}{bin} {version}\n\
{author-section}\
{about-section}\n\
USAGE\n    {usage}\n\
\n\
{all-args}{after-help}")).get_matches();


    let config_file = matches.value_of("config").map(path::PathBuf::from).unwrap_or_else(get_default_config);
    let plugin_folder = matches.value_of("plugins").map(path::PathBuf::from).unwrap_or_else(get_default_plugin_directory);
    let print_sample_config = matches.is_present("sampleconfig");
    CommandlineParameters { config_file, plugin_folder , print_sample_config}
}

/// Searches for the config file in XDG paths. If not found there, instead the
/// /etc/swaystatus/config path is returned.
fn get_default_config() -> path::PathBuf {
    if let Some(mut xdg) = dirs::config_dir() {
        xdg.push("swaystatus/config");
        if xdg.exists() {
            return xdg;
        }
    }
    //no XDG data dir config file found.
    path::PathBuf::from("/etc/swaystatus/config")
}

fn get_hardcoded_default_library_path() -> &'static str {
    option_env!("DEFAULT_PLUGIN_DIR").unwrap_or("/usr/lib/swaystatus/")
}

/// Searches for plugins. First checks the user's ~/.local/lib/swaystatus/ folder, then the folder
/// passed as build parameter (DEFAULT_PLUGIN_DIR)
fn get_default_plugin_directory() -> path::PathBuf {
    if let Some(mut user_folder) = dirs::home_dir() {
        user_folder.push(".local/lib/swaystatus/");
        if user_folder.is_dir() {
            return user_folder;
        }
    }

    path::PathBuf::from(get_hardcoded_default_library_path())
}
