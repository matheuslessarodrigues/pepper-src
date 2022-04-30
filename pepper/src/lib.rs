pub mod application;
pub mod buffer;
pub mod buffer_history;
pub mod buffer_position;
pub mod buffer_view;
pub mod client;
pub mod command;
pub mod config;
pub mod cursor;
pub mod editor;
pub mod editor_utils;
pub mod events;
pub mod glob;
pub mod help;
pub mod mode;
pub mod navigation_history;
pub mod pattern;
pub mod picker;
pub mod platform;
pub mod plugin;
pub mod serialization;
pub mod syntax;
pub mod theme;
pub mod ui;
pub mod word_database;

pub const DEFAULT_CONFIGS: ResourceFile = ResourceFile {
    name: "default_configs.pepper",
    content: include_str!("../rc/default_configs.pepper"),
};
pub const DEFAULT_SYNTAXES: ResourceFile = ResourceFile {
    name: "default_syntaxes.pepper",
    content: include_str!("../rc/default_syntaxes.pepper"),
};

#[derive(Clone, Copy)]
pub struct ResourceFile {
    pub name: &'static str,
    pub content: &'static str,
}

pub struct ArgsConfig {
    pub path: String,
    pub suppress_file_not_found: bool,
}

#[derive(Default)]
pub struct Args {
    pub version: bool,
    pub session: Option<String>,
    pub print_session: bool,
    pub as_focused_client: bool,
    pub quit: bool,
    pub server: bool,
    pub configs: Vec<ArgsConfig>,
    pub files: Vec<String>,
}

fn print_version() {
    let name = env!("CARGO_PKG_NAME");
    let version = env!("CARGO_PKG_VERSION");
    println!("{} version {}", name, version);
}

fn print_help() {
    print_version();
    println!("{}", env!("CARGO_PKG_DESCRIPTION"));
    println!();
    println!("usage: pepper [<options...>] [<files...>]");
    println!();
    println!("  files: file paths to open as a buffer (clients only)");
    println!("         you can append ':<line>[,<column>]' to open it at that position");
    println!();
    println!("options:");
    println!();
    println!("  -h, --help               prints help and quits");
    println!("  -v, --version            prints version and quits");
    println!("  -s, --session            overrides the session name to connect to");
    println!("  --print-session          prints the computed session name and quits");
    println!("  --as-focused-client      sends events as if it was the currently focused client");
    println!("  --quit                   sends a `quit` event on start");
    println!("  --server                 only run as server");
    println!("  -c, --config[!]          sources config file at path (repeatable) (server only)");
    println!("                           with `!` it will suppress the 'file not found' error");
}

impl Args {
    pub fn parse() -> Self {
        fn error(message: std::fmt::Arguments) -> ! {
            eprintln!("{}", message);
            std::process::exit(0);
        }

        fn arg_to_str(arg: &std::ffi::OsString) -> &str {
            match arg.to_str() {
                Some(arg) => arg,
                None => error(format_args!("could not parse arg {:?}", arg)),
            }
        }

        let mut args = std::env::args_os();
        args.next();

        let mut parsed = Args::default();
        while let Some(arg) = args.next() {
            let arg = arg_to_str(&arg);
            match arg {
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                "-v" | "--version" => {
                    print_version();
                    std::process::exit(0);
                }
                "-s" | "--session" => match args.next() {
                    Some(arg) => {
                        let arg = arg_to_str(&arg);
                        if !arg.chars().all(char::is_alphanumeric) {
                            error(format_args!(
                                "invalid session name '{}'. it can only contain alphanumeric characters", arg
                            ));
                        }
                        parsed.session = Some(arg.into());
                    }
                    None => error(format_args!("expected session after {}", arg)),
                },
                "--print-session" => parsed.print_session = true,
                "--as-focused-client" => parsed.as_focused_client = true,
                "--quit" => parsed.quit = true,
                "--server" => parsed.server = true,
                "-c" | "-c!" | "--config" | "--config!" => {
                    let suppress_file_not_found = arg.ends_with('!');
                    match args.next() {
                        Some(arg) => {
                            let arg = arg_to_str(&arg);
                            parsed.configs.push(ArgsConfig {
                                path: arg.into(),
                                suppress_file_not_found,
                            });
                        }
                        None => error(format_args!("expected config path after {}", arg)),
                    }
                }
                "--" => {
                    for arg in args.by_ref() {
                        let arg = arg_to_str(&arg);
                        parsed.files.push(arg.into());
                    }
                }
                _ => {
                    if arg.starts_with('-') {
                        error(format_args!("invalid option '{}'", arg));
                    } else {
                        parsed.files.push(arg.into());
                    }
                }
            }
        }

        parsed
    }
}

#[cfg(target_os = "windows")]
#[path = "platforms"]
mod platform_impl {
    #[path = "windows.rs"]
    pub mod sys;
}

#[cfg(target_os = "linux")]
#[path = "platforms"]
mod platform_impl {
    #[path = "linux.rs"]
    pub mod sys;
}

#[cfg(target_os = "macos")]
#[path = "platforms"]
mod platform_impl {
    #[path = "bsd.rs"]
    pub mod sys;
}

#[cfg(any(
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
))]
#[path = "platforms"]
mod platform_impl {
    #[path = "bsd.rs"]
    pub mod sys;
}

#[cfg(not(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
)))]
mod platform_impl {
    use super::*;
    pub mod sys {
        use super::*;
        pub fn try_launching_debugger() {}
        pub fn main(_: application::ApplicationConfig) {}
    }
}

pub fn init(config: &application::ApplicationConfig) {
    use std::{fs, io, mem::MaybeUninit, panic};

    static mut ORIGINAL_PANIC_HOOK: MaybeUninit<Box<dyn Fn(&panic::PanicInfo) + Sync + Send>> =
        MaybeUninit::uninit();
    unsafe { ORIGINAL_PANIC_HOOK = MaybeUninit::new(panic::take_hook()) };

    static mut ON_PANIC_CONFIG: MaybeUninit<application::OnPanicConfig> = MaybeUninit::uninit();
    unsafe { ON_PANIC_CONFIG = MaybeUninit::new(config.on_panic_config) };

    panic::set_hook(Box::new(|info| unsafe {
        let config = ON_PANIC_CONFIG.assume_init_ref();

        if let Some(path) = config.write_info_to_file {
            if let Ok(mut file) = fs::File::create(path) {
                use io::Write;
                let _ = writeln!(file, "{}", info);
            }
        }

        if config.try_attaching_debugger {
            platform_impl::sys::try_launching_debugger();
        }

        let hook = ORIGINAL_PANIC_HOOK.assume_init_ref();
        hook(info);
    }));
}

pub fn run(config: application::ApplicationConfig) {
    init(&config);
    platform_impl::sys::main(config);
}
