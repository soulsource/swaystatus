use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct SwaystatusConfig {
    separator : Option<String>, //Separator character between elements.
    plugin_path : Option<String>, //path to load plugins from. If unset, hardcoded value is used.
    elements : Option<Vec<(String,String)>>, //plugins to display and their config section.
}

fn create_default_config() -> SwaystatusConfig {
   return SwaystatusConfig{
       separator : Some(String::from(", ")), 
       plugin_path : Some(String::from("")), 
       //elements : Some(Vec::new())}; 
       elements : Some(vec!((String::from("time"), String::from("format = \"yyyy-mm-dd hh-mm-ss\""))))
   };
}

pub enum SwaystatusConfigErrors
{
    FileNotFound,
    ParsingError {
        message : String
    }
}

pub fn print_sample_config() {
    let default_config = create_default_config();
    let output = toml::to_string(&default_config).unwrap();
    print!("{}", output);
}

pub fn read_config(path : &std::path::Path) -> Result<SwaystatusConfig,SwaystatusConfigErrors> {
    let config_file = match std::fs::read_to_string(path) {
        Ok(x) => x,
        Err (_) => return Err(SwaystatusConfigErrors::FileNotFound)
    };
    let result = match toml::from_str(&config_file) {
        Ok(x) => x,
        Err(e) => return Err(SwaystatusConfigErrors::ParsingError{message: e.to_string()})
    };

    return Ok(result);

}
