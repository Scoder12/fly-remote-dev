use color_eyre::eyre::{eyre, WrapErr};
use procfs::net::TcpState;
use std::{
    convert::TryInto,
    env,
    ffi::OsStr,
    fs::{create_dir_all, write, File},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread::sleep,
    time::{Duration, Instant},
};

const SSH_PORT: u16 = 22;
const CODE_SERVER_PORT: u16 = 8080;
const CODE_SERVER_PATH: &'static str = "/usr/lib/code-server";

const CUSTOM_FONTS_BEGIN: &'static str = "/* CUSTOM FONTS BEGIN */";
const CUSTOM_FONTS_END: &'static str = "/* CUSTOM FONTS END */";
const CUSTOM_FONTS_PATCH: &'static str = include_str!("fonts-patch.css");
const EXTENSIONS_TXT: &'static str = include_str!("extensions.txt");

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let home: PathBuf = env::var_os("HOME")
        .ok_or(eyre!("HOME var not set"))?
        .try_into()
        .wrap_err("$HOME is invalid")?;
    let home = home
        .canonicalize()
        .wrap_err("Unable to canonicalize $HOME")?;
    let xdg_config_home: PathBuf = env::var_os("XDG_CONFIG_HOME")
        .map(|p| p.into())
        .unwrap_or_else(|| home.clone().join(".config"));
    let xdg_data_home: PathBuf = env::var_os("XDG_DATA_HOME")
        .map(|p| p.into())
        .unwrap_or_else(|| home.clone().join(".local").join("share"));

    let ssh_pubkey = env::var("SSH_PUBKEY").wrap_err("SSH_PUBKEY var invalid")?;
    let auth_keys_file = home.join(".ssh").join("authorized_keys");
    create_dir_all(auth_keys_file.parent().unwrap())?;
    write(auth_keys_file, ssh_pubkey)?;

    let code_server_password =
        env::var("CODE_SERVER_PASS").wrap_err("CODE_SERVER_PASS var invalid")?;
    let code_server_config_path: PathBuf = xdg_config_home.join("code-server").join("config.yaml");
    create_dir_all(code_server_config_path.parent().unwrap())?;
    write(
        code_server_config_path,
        format!(
            "bind-addr: 0.0.0.0:{}\n\
            auth: password\n\
            cert: false\n",
            CODE_SERVER_PORT,
        ),
    )?;

    let workbench_css_path = PathBuf::from(CODE_SERVER_PATH)
        .join("lib")
        .join("vscode")
        .join("out")
        .join("vs")
        .join("workbench")
        .join("workbench.web.main.css");
    assert!(Command::new("sudo")
        .arg("chmod")
        .arg("a+w")
        .arg::<&OsStr>(workbench_css_path.as_ref())
        .status()?
        .success());

    let contents = std::fs::read_to_string(&workbench_css_path)?;
    let (patch_begin, patch_end) = if let Some(prefix_begin) = contents.find(CUSTOM_FONTS_BEGIN) {
        let prefix_end = contents
            .find(CUSTOM_FONTS_END)
            .ok_or_else(|| eyre!("Found CUSTOM_FONTS_BEGIN but missing CUSTOM_FONTS_END"))?;
        if prefix_end <= prefix_begin {
            return Err(eyre!("CUSTOM_FONTS_END after CUSTOM_FONTS_BEGIN"));
        }
        (prefix_begin, prefix_end + CUSTOM_FONTS_END.len())
    } else {
        (0, 0)
    };
    write!(
        File::options()
            .write(true)
            .open::<&Path>(workbench_css_path.as_ref())?,
        "{}{}{}{}{}",
        &contents[..patch_begin],
        CUSTOM_FONTS_BEGIN,
        CUSTOM_FONTS_PATCH,
        CUSTOM_FONTS_END,
        &contents[patch_end..]
    )?;
    Command::new("sh")
        .arg("-c")
        .arg("set -x; grep -rl \"style-src 'self' 'unsafe-inline'\" . \
            | sudo xargs sed -i \"s/style-src 'self' 'unsafe-inline'/style-src 'self' 'unsafe-inline' fonts.googleapis.com/g\"")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    Command::new("sh")
        .arg("-c")
        .arg("grep -rl \"font-src 'self' blob:\" . \
            | sudo xargs sed -i \"s/font-src 'self' blob:/font-src 'self' blob: fonts.gstatic.com/g\"")
        .stdin(Stdio::null())
        .status()?;

    let settings_json = include_bytes!("settings.json");
    let settings_json_path = xdg_data_home
        .join("code-server")
        .join("Machine")
        .join("settings.json");
    create_dir_all(settings_json_path.parent().unwrap())?;
    write(settings_json_path, settings_json)?;

    let status = Command::new("sudo")
        .arg("service")
        .arg("ssh")
        .arg("start")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    assert!(status.success());

    Command::new("code-server")
        .env("CS_DISABLE_GETTING_STARTED_OVERRIDE", "1")
        .env("PASSWORD", code_server_password)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    let extensions: Vec<&str> = EXTENSIONS_TXT.lines().filter(|l| !l.is_empty()).collect();
    for ext in extensions.into_iter() {
        println!("Installing extension: {:#?}", ext);
        Command::new("code-server")
            .arg("--install-extension")
            .arg(ext)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
    }

    let mut last_activity = Instant::now();

    loop {
        if count_conns()? > 0 {
            last_activity = Instant::now();
        } else {
            let idle_time = last_activity.elapsed();
            println!("Idle for {idle_time:?}");
            if idle_time > Duration::from_secs(60) {
                println!("Stopping machine. Goodbye!");
                std::process::exit(0)
            }
        }
        sleep(Duration::from_secs(5));
    }
}

fn count_conns() -> color_eyre::Result<usize> {
    Ok(procfs::net::tcp()?
        .into_iter()
        // don't count listen, only established
        .filter(|entry| matches!(entry.state, TcpState::Established))
        .filter(|entry| matches!(entry.local_address.port(), SSH_PORT | CODE_SERVER_PORT))
        .count())
}
