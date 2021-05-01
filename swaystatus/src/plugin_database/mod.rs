use std::collections::HashMap;
use super::plugin;
use libloading::{Library};
use gettextrs::*;

pub struct PluginDatabase<'p> {
   plugins : HashMap<String, Box<dyn plugin::SwayStatusModule + 'p>>
}

impl<'a> PluginDatabase<'a> {
    pub fn get_plugin<'b>(&'a self, name : &'b str) -> Option<&(dyn plugin::SwayStatusModule + 'a)> {
        self.plugins.get(name).map(|x| &**x)
    }
    pub fn get_name_and_plugin_iterator(&'a self) -> impl Iterator<Item = (&'a String,&'a Box<dyn plugin::SwayStatusModule +'a>)> + 'a {
        self.plugins.iter()
    }
    pub fn new<'b : 'a>(libs : &'b Libraries) -> PluginDatabase<'a> {
        PluginDatabase {
            plugins : libs.libs.iter().filter_map(|(path, lib)| {
                match get_plugin_from_library(lib) {
                    Ok(x) => Some((String::from(x.get_name()), x)),
                    Err(y) => {
                        let lib_name = path.to_string_lossy();
                        match y {
                            PluginLoadingError::MissingVersionInformation => {
                                eprintln!("{}", gettext!("Failed to load library {}, no version information found.", lib_name));
                            },
                            PluginLoadingError::WrongPluginVersion { expected, version } => {
                                eprintln!("{}", gettext!("Failed to load library {}, it was built with the wrong plugin version. Expected version {}, found version {}", lib_name, expected, version));
                            }
                            PluginLoadingError::WrongRustcVersion { expected, version } => {
                                eprintln!("{}", gettext!("Failed to load library {}, it was built with a different Rust version. Since there is no ABI stability guaranteed, this safeguard is required. Please make sure this program and all plugins use the same compiler version. Expected the Rust version {}, found version {}", lib_name, expected, version));
                            }
                            PluginLoadingError::NoConstructor => {

                                eprintln!("{}", gettext!("Failed to load library {}, it does not export the _swaystatus_module_create() function.", lib_name));
                            }
                        }
                        None
                    }
                }

            }).collect()
        }
    }
}

pub struct Libraries {
    libs : Vec<(std::path::PathBuf,Library)>
}
impl Libraries {
    pub fn load_from_folder(path : &std::path::Path) -> Result<Libraries, std::io::Error> {
        Ok(Libraries {
            libs : path.read_dir()?.filter_map(|f| {
                match f {
                    Err(e) => {
                        eprintln!("{}", gettext!("File I/O error while iterating libraries: {}", e.to_string()));
                        None
                    },
                    Ok(d) => unsafe {
                        let p = d.path();
                        match libloading::Library::new(&p) {
                            Ok(x) => Some((p,x)),
                            Err(_) => {
                                eprintln!("{}", gettext!("Failed to load as library: {}", d.path().display()));
                                None
                            }
                        }
                    }
                }
            }).collect()
        })
    }
}
enum PluginLoadingError {
    MissingVersionInformation,
    WrongPluginVersion { expected: &'static str, version : String },
    WrongRustcVersion { expected: &'static str, version : String },
    NoConstructor
}

fn get_plugin_from_library<'p>(lib : &'p Library) -> Result<Box<dyn plugin::SwayStatusModule + 'p>,PluginLoadingError> {
    unsafe {
        let version_getter = lib.get::<unsafe extern fn() -> *const str>(b"_swaystatus_module_version");
        let rustc_version_getter = lib.get::<unsafe extern fn() -> *const str>(b"_swaystatus_rustc_version");
        if version_getter.is_err() || rustc_version_getter.is_err() {
            return Err(PluginLoadingError::MissingVersionInformation);
        }
        let found_version = &*(version_getter.unwrap())();
        let found_rustc_version = &*(rustc_version_getter.unwrap())();
        if found_version != swaystatus_plugin::MODULE_VERSION {
            return Err(PluginLoadingError::WrongPluginVersion { expected: swaystatus_plugin::MODULE_VERSION, version : String::from(found_version) });
        }
        if found_rustc_version != swaystatus_plugin::RUSTC_VERSION {
            return Err(PluginLoadingError::WrongRustcVersion { expected : swaystatus_plugin::RUSTC_VERSION, version : String::from(found_rustc_version)});
        }
        if let Ok(constructor) = lib.get::<unsafe extern fn() ->*mut dyn swaystatus_plugin::SwayStatusModule>(b"_swaystatus_module_create") {
            return Ok(Box::from_raw(constructor()));
        }
        Err(PluginLoadingError::NoConstructor)
   }
}

#[cfg(test)]
pub mod test_helper; 
