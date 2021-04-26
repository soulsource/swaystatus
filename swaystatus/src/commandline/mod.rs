extern crate clap;
extern crate dirs;
use clap::*;
use std::path;
use gettextrs::*;

pub enum PluginHelpOption{
    All,
    List(Vec<String>)
}
pub enum CommandlineAction {
    Run {
        config_file : path::PathBuf,
    },
    PrintSampleConfig,
    PluginHelp(PluginHelpOption),
    ListPlugins
}
pub struct CommandlineParameters{
    pub plugin_folder : path::PathBuf,
    pub action : CommandlineAction
}

/// Gets the config and plugin paths. Either from command line or from hardcoded defaults.
pub fn parse_commandline() -> CommandlineParameters {
    //needed for lifetime reasons...
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(&*gettext("A simple status text app, inspired by i3status"))
        .arg(
            Arg::new("help")
            .short('h')
            .long("help")
            .about(&*gettext("Prints help information."))
            .global(true))
        .arg(
            Arg::new("version")
            .short('v')
            .long("version")
            .about(&*gettext("Prints version information."))
            .global(true))
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
            .short('s')
            .about(&*gettext("Prints a sample config file. Beware that the contents of the sample file depend on the loaded plugins, so don't forget to supply the plugins parameter as needed."))
            .takes_value(false)
            .conflicts_with_all(&["pluginhelp","pluginlist"]))
        .arg(
            Arg::new("pluginhelp")
            .long("plugin-help")
            .short('g')
            .value_name(&*gettext("PLUGINS"))
            .about(&*gettext("Prints plugin help messages. Either for a given list of plugins, or if no list given, for all loadable plugins."))
            .min_values(0)
            .setting(ArgSettings::MultipleValues)
            .conflicts_with("pluginlist"))
        .arg(
            Arg::new("pluginlist")
            .long("list-plugins")
            .short('l')
            .about(&*gettext("Prints a list of plugin names in the plugin folder."))
            .takes_value(false))
        .after_help(&*gettext!("If no config path is given, the code looks for the \"swaystatus/config\" file in your XDG config folder (typically \"$HOME/.config/\"). If that lookup fails, loading of \"/etc/swaystatus/config\" is attempted. Similarly, if no plugin folder is given, first the existence of a folder named \"$HOME/.local/lib/swaystatus\" is checked. If this folder does not exist, a default path set at compile time is used, which in your case is \"{}\"." , get_hardcoded_default_library_path()))
        .help_template(&*gettext("\
{before-help}{bin} {version}\n\
{author}\n
{about}\n\
USAGE\n    {usage}\n\
\n\
FLAGS:
{flags}\n
OPTIONS:
{options}\n
{after-help}")).get_matches();

    let plugin_folder = matches.value_of("plugins").map(path::PathBuf::from).unwrap_or_else(get_default_plugin_directory);
    if matches.is_present("sampleconfig") {
        CommandlineParameters { plugin_folder, action : CommandlineAction::PrintSampleConfig }
    }
    else if matches.is_present("pluginlist") {
        CommandlineParameters {plugin_folder, action : CommandlineAction::ListPlugins }
    }
    else if let Some(iter) = matches.values_of("pluginhelp") {
        CommandlineParameters {plugin_folder, action : CommandlineAction::PluginHelp(
            if iter.len() == 0 { PluginHelpOption::All }
            else {PluginHelpOption::List(iter.map(String::from).collect())}
        )}
    }
    else {
        let config_file = matches.value_of("config").map(path::PathBuf::from).unwrap_or_else(get_default_config);
        CommandlineParameters {plugin_folder, action : CommandlineAction::Run { config_file }}
    }
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
