pub use swaystatus_plugin as plugin;
mod config;
mod plugin_database;
mod communication;
mod signalhandler;
mod commandline;

extern crate gettextrs;
use gettextrs::*;
use crossbeam_utils::thread;
use std::sync::mpsc;

use commandline::CommandlineAction;
#[cfg(test)]
pub mod test_plugin;

#[global_allocator]
static GLOBAL : std::alloc::System = std::alloc::System;

fn main() {
    let text_domain = match dirs::data_dir() {
        Some (p) => TextDomain::new("swaystatus").prepend("target").push(p),
        None => TextDomain::new("swaystatus").prepend("target")
    };
    if let Err(_e) = text_domain.init() {
        eprintln!("Localization could not be loaded. Will use English instead.");
    }
    let commandline_parameters = commandline::parse_commandline();
    match commandline_parameters.action {
        CommandlineAction::PrintSampleConfig => { 
            print_sample_config(&commandline_parameters.plugin_folder);
        }
        CommandlineAction::ListPlugins => {
            list_plugins(&commandline_parameters.plugin_folder);
        }
        CommandlineAction::PluginHelp(list) => {
            print_plugin_help(&commandline_parameters.plugin_folder, list);
        }
        CommandlineAction::Run { config_file } => {
            while !core_loop(&commandline_parameters.plugin_folder, &config_file) {}
        }
    }

}

/// Actually the main() function. Factored out so we can restart without actually restaring.
/// Because some people might expect that SIGHUP triggers a reload, and it's trivial to implement.
fn core_loop(plugin_path : &std::path::Path, config_path : &std::path::Path) -> bool {
    //Read plugins first (needed for config deserialization, given the config files has
    //plugin config as well...
    let libraries = match plugin_database::Libraries::load_from_folder(plugin_path) {
        Ok(x) => x,
        Err(e) => {
            print_plugin_load_error(e,plugin_path);
            return true;
        }
    };
    let plugins = plugin_database::PluginDatabase::new(&libraries); 

    let (elements, main_config) = match config::SwaystatusConfig::read_config(config_path, &plugins) {
        Ok(x) => (x.elements.unwrap_or_default(), x.settings.unwrap_or_default()),
        Err(e) => { print_config_error(e); return true;}
    };

    if elements.is_empty() {
        eprintln!("{}", gettext("No elements set up in configuration. Nothing to display."));
        return true;
    }

    let (sender_from_plugins, receiver_from_plugins) = mpsc::channel();

    let (runnables, senders_to_plugins) : (Vec<_>, Vec<_>) = elements.iter().enumerate().map(|(i,x)| {
        let s = communication::SenderToMain { 
            sender : sender_from_plugins.clone(),
            element_number : i,
        };
        x.get_instance().make_runnable(Box::new(s))
    }).unzip();

    //mutable array into which we store our updated texts.
    let mut texts = Vec::with_capacity(elements.len());
    texts.resize(elements.len(),String::new());
    assert_eq!(texts.len(), runnables.len());
    assert_eq!(texts.len(), senders_to_plugins.len());
    assert_eq!(elements.len(), runnables.len());

    let mut should_restart = false;

    // Main everything is ready for the big main loop. Let's spawn the threads!
    if let Err(_e) = thread::scope(|s| {
        signalhandler::handle_signals(s, sender_from_plugins);
        for runnable in runnables {
            s.spawn(move |_| {
                runnable.run();
            });
        }

        while let Ok(msg) = receiver_from_plugins.recv() {
            match msg {
                communication::Message::Internal(i) => {
                    if let communication::InternalMessage::Reload = i {
                        should_restart = true;
                    }
                    forward_to_all_plugins(&senders_to_plugins,&elements, i);
                },
                communication::Message::External{text, element_number} => {
                    handle_message_from_element(&mut texts, &elements[element_number].get_name(), element_number, text);
                    print_texts(&texts, &main_config, &elements);
                },
                communication::Message::ThreadCrash{element_number} => {
                    handle_crash_from_element(&mut texts, &elements[element_number].get_name(), element_number);
                    print_texts(&texts, &main_config, &elements);
                }
            }
        }


    }) {
        //unwinding across plugin boundaries is a _bad_ idea. Unless we want our core dumped, that
        //is. The documentation only mentions that unwinding across C functions doesn't work, but
        //that seems to also be true for dynamically loaded Rust libs... That's why we can only
        //print a general error here.
        eprintln!("{}", gettext("At least one of the plugins panicked. For details please check the (hopefully existing) previous error messages."));
    }

    !should_restart
}

//-----------------------------------------------------------------------------
//Helpers

fn print_config_error(e : config::SwaystatusConfigErrors) {
    match e {
        config::SwaystatusConfigErrors::FileNotFound => {
            eprintln!("{}", gettext("The configuration file could not be read. Nothing to do."));
        },
        config::SwaystatusConfigErrors::ParsingError {message} => {
            eprintln!("{}", gettext!("The parser for the config file returned an error: {}", message));
        }
    }
}

fn forward_to_all_plugins<'p>(senders : &[Box<dyn plugin::MsgMainToModule + 'p>], elements : &[config::SwaystatusPluginConfig], message : communication::InternalMessage) {
    match message {
        communication::InternalMessage::Quit | communication::InternalMessage::Reload => {
            for (i, sender) in senders.iter().enumerate() {
                if sender.send_quit().is_err() {
                    eprintln!("{}", gettext!("Tried to tell a plugin to quit, but that plugin seems to no longer listen to messages. Either that plugin has already terminated, or it's stuck. In the latter case a clean exit is impossible, you'll need to kill this process. The offending element is element number {} from plugin {}.", i, elements[i].get_name()));
                }
            }
        },
        communication::InternalMessage::Refresh => {
            for (i,sender) in senders.iter().enumerate() {
                if sender.send_refresh().is_err() {
                    eprintln!("{}", gettext!("Tried to tell a plugin to refresh, but it doesn't listen any more. Either the plugin already terminated, or it is stuck. The offending element is element number {} from plugin {}.", i, elements[i].get_name()));
                }
            }
        }
    }
}

fn handle_message_from_element(texts : &mut Vec<String>, plugin : &str, element_number : usize, message : Result<String, plugin::PluginError>) {
    match message {
        Ok(t) => texts[element_number] = t,
        Err(e) => match e {
            plugin::PluginError::PrintToStdErr(t) => eprintln!("{}", gettext!("Element number {} (plugin: {}) sent an error message: {}",element_number, plugin,t)),
            plugin::PluginError::ShowInsteadOfText(t) => {
                eprintln!("{}", gettext!("Element number {} (plugin: {}) sent an error message: {}",element_number, plugin,t));
                texts[element_number] = t;
            }
        }
    }
}

fn print_texts(texts : &[String], settings : &config::SwaystatusMainConfig, element_settings : &[config::SwaystatusPluginConfig]) {
    //Once we do more than just printing, we might want a more advanced code here...
    let separators = std::iter::once("").chain(std::iter::repeat(&settings.separator[..]));
    let before_texts = element_settings.iter().map(|x| &x.get_non_plugin_settings().before_text);
    let after_texts = element_settings.iter().map(|x| &x.get_non_plugin_settings().after_text);
    let text_with_after = texts.iter().zip(after_texts);
    let complete_text = before_texts.zip(text_with_after);

    let final_iterator = separators.zip(complete_text);

    for (separator, (before,(text,after))) in final_iterator {
        print!("{}{}{}{}",separator,before,text,after);
    }
    println!(); //Previosly there was an explicit flush here, but printnl should do that for us.
}

fn handle_crash_from_element(texts : &mut Vec<String>, name : &str, element_number : usize) {
    texts[element_number] = gettext("<plugin crashed>");
    eprintln!("{}", gettext!("The plugin {} crashed while displaying element number {}. Please see the plugin's panic message above for details.",name, element_number));
}

fn print_plugin_load_error(e : std::io::Error, plugin_path : &std::path::Path) {
    eprintln!("{} {}", gettext!("Tried to load plugins from folder \"{}\", but failed. You might want to set a plugin directory on the command line. The actual error was:", plugin_path.display()), e);
}

fn print_sample_config(plugin_path : &std::path::Path) {
    let libraries = match plugin_database::Libraries::load_from_folder(plugin_path) {
        Ok(x) => x,
        Err(e) => {
            print_plugin_load_error(e,plugin_path);
            return;
        }
    };
    let plugins = plugin_database::PluginDatabase::new(&libraries); 

    config::SwaystatusConfig::print_sample_config(&plugins);
}

fn list_plugins(plugin_path : &std::path::Path) {
    let libraries = match plugin_database::Libraries::load_from_folder(plugin_path) {
        Ok(x) => x,
        Err(e) => {
            print_plugin_load_error(e,plugin_path);
            return;
        }
    };
    let plugins = plugin_database::PluginDatabase::new(&libraries);
    for (name, _) in plugins.get_name_and_plugin_iterator() {
        println!("{}", name);
    }
}

fn print_plugin_help(plugin_path : &std::path::Path, list : commandline::PluginHelpOption) {
    let libraries = match plugin_database::Libraries::load_from_folder(plugin_path) {
        Ok(x) => x,
        Err(e) => {
            print_plugin_load_error(e,plugin_path);
            return;
        }
    };
    let plugins = plugin_database::PluginDatabase::new(&libraries);
    match list {
        commandline::PluginHelpOption::All => {
            for (n, p) in plugins.get_name_and_plugin_iterator() {
                println!("{}\n",gettext!("Plugin: \"{}\"",n));
                p.print_help();
                println!("\n\n");
            }
        }
        commandline::PluginHelpOption::List(l) => {
            for name in l {
                println!("{}\n",gettext!("Plugin: \"{}\"",name));
                if let Some(p) = plugins.get_plugin(&name) {
                    p.print_help();
                }
                else {
                    println!("{}", gettext!("Plugin {} not found.", name));
                }
                println!("\n\n");
            }
        }
    }
}
