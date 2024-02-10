# swaystatus

This is a small status bar application that I write/maintain because I use it myself. You probably do not want to use this.
The only reason I published it is because I wanted to have it available somewhere online, for reference.

## Information for users

Let me start with the important part: For most users this program is not a good choice. Mainly because it is rather limited feature-wise, and I have no plans to extend it beyond my own personal needs.
So, if you just want to have a nice status bar for sway or i3, check out [i3status-rust](https://github.com/greshake/i3status-rust) instead. It's very similar, but has way more features.

However, if you want something that's similar to i3status-rust, but with a fully modular plugin architecture, then this might be of interest to you. Instead of having to choose at build-time which features should be enabled, with this program you can just add/remove features by installing/removing plugins.

### Setup

This repo is a cargo workspace. Just clone it, and run `cargo build --release`. That will generate a swaystatus executable, as well as a bunch of plugin .so files in the target/release folder. To exclude certain plugins from a build (for instance), use `cargo build --release --workspace --exclude <whatever>`. This is useful if you don't have all dependencies of all plugins on your machine (for instance swaystatus-pulse needs pulseaudio, but if you don't have pulseaudio, you likely won't need swaystatus-pulse either). Then toss the binary and the plugin files at folders that are convenient to you.

For more information, run `swaystatus --help`. The last few lines of help output detail where plugins are searched by default, and how you can override the plugin path.

#### Configuration

You'll need a config file. The easiest way to get started is to run `swaystatus --print-sample-config` and start from there. Each plugin has its own configuration, of course, so you might get some valuable insights from `swaystatus --plugin-help` too.

#### Launching

Unless you put the libraries and the configuration exactly where this program expects them to be, you'll want to pass both paths: `swaystatus -p <plugin-folder> -c <config-file>`.


## Info for developers

First things first again: Feel free to extend this, PRs are welcome. However, my motivation to work on this tool is limited, so, please keep your expectations low. If you want, feel free to fork and do whatever you like - but, if possible, keep the plugin interface compatible. That way people can easily mix plugins from different forks.

### Making a new plugin

Plugins are Rust crates that compile to shared libraries. Check out the Cargo.toml file of the clock plugin to get started. The plugin interface is in the swaystatus-plugin crate. It's rather straightforward to implement. You'll need three objects, a `SwayStatusModule`, a `SwayStatusModuleInstance` and a `SwayStatusModuleRunnable`. The Module describes the plugin itself, and offers ways to spawn a `SwayStatusModuleInstance` (either with default values, or with deserialized configuration). The `SwayStatusModuleInstance` stores the configuration, and allows spawning of a `SwayStatusModuleRunnable`, which does the work. Communication between plugin and main program is done via traits too. The main program passes a means to send data to it when spawning the runnable, and wants to get a way to pass messages to the runnable in return. Check out the clock plugin for a simple channel-based approach, and the pulse (or the WIP alsa) plugins for more exotic solutions. Happy hacking.